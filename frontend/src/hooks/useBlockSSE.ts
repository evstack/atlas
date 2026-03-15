import { useCallback, useEffect, useRef, useState } from 'react';
import type { Block } from '../types';
import { API_BASE_URL } from '../api/client';

export interface NewBlockEvent {
  block: Block;
}

export interface DaUpdateEvent {
  block_number: number;
  header_da_height: number;
  data_da_height: number;
}

export interface DaBatchEvent {
  updates: DaUpdateEvent[];
}

export type DaSubscriber = (updates: DaUpdateEvent[]) => void;

export interface BlockSSEState {
  latestBlock: NewBlockEvent | null;
  latestDaUpdate: DaUpdateEvent | null;
  height: number | null;
  connected: boolean;
  error: string | null;
  bps: number | null;
  subscribeDa: (cb: DaSubscriber) => () => void;
}

type BlockLog = { num: number; ts: number }[];
type QueuedBlock = { event: NewBlockEvent; receivedAt: number };

const MIN_DRAIN_INTERVAL_MS = 30;
const MAX_DRAIN_INTERVAL_MS = 500;
const STREAM_BUFFER_BLOCKS = 3;
const MAX_BUFFER_WAIT_MS = 320;
const MAX_BURST_LEAD_IN_MS = 320;

/**
 * Compute bps from a rolling log of (blockNumber, blockTimestamp) samples.
 * Returns the rate from the first sample whose span >= minSpan, or from the
 * oldest sample if its span >= fallbackMinSpan. Returns null if insufficient data.
 */
function computeBpsFromLog(log: BlockLog, minSpan: number, fallbackMinSpan: number): number | null {
  if (log.length < 2) return null;
  const newest = log[log.length - 1];
  for (let i = 0; i < log.length - 1; i++) {
    const span = newest.ts - log[i].ts;
    if (span >= minSpan) return (newest.num - log[i].num) / span;
    if (i === 0 && span >= fallbackMinSpan) return (newest.num - log[0].num) / span;
  }
  return null;
}

function getDrainInterval(baseInterval: number, queueLength: number): number {
  let interval = baseInterval;

  // Speed up gently when backlog builds, but avoid collapsing back into bursts.
  if (queueLength > 24) interval *= 0.62;
  else if (queueLength > 12) interval *= 0.72;
  else if (queueLength > 6) interval *= 0.82;
  else if (queueLength > 2) interval *= 0.9;

  return Math.max(MIN_DRAIN_INTERVAL_MS, interval);
}

/**
 * Connects to the SSE endpoint and delivers block events one-by-one.
 *
 * SSE events can still arrive in bursts when the backend indexes a backlog.
 * React batching would collapse rapid setState calls, so we buffer into a
 * ref-based queue and drain one at a time at a steadier visual cadence.
 *
 * The drain interval is derived from the chain's true block time, computed
 * from on-chain block timestamps (not arrival/indexed times). We collect a
 * rolling window of (blockNumber, blockTimestamp) samples and compute
 * bps = deltaBlocks / deltaTimestamp. Since timestamps have second-level
 * granularity, we require at least 2 seconds of block-time span for accuracy.
 * This makes the visual cadence match the chain's actual speed, regardless of
 * indexer lag, network batching, or SSE delivery timing.
 */
export default function useBlockSSE(): BlockSSEState {
  const [latestBlock, setLatestBlock] = useState<NewBlockEvent | null>(null);
  const [latestDaUpdate, setLatestDaUpdate] = useState<DaUpdateEvent | null>(null);
  const [height, setHeight] = useState<number | null>(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bps, setBps] = useState<number | null>(null);
  const daSubscribersRef = useRef<Set<DaSubscriber>>(new Set());
  const subscribeDa = useCallback((cb: DaSubscriber) => {
    daSubscribersRef.current.add(cb);
    return () => { daSubscribersRef.current.delete(cb); };
  }, []);
  const esRef = useRef<EventSource | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const queueRef = useRef<QueuedBlock[]>([]);
  const drainTimerRef = useRef<number | null>(null);
  const lastDrainAtRef = useRef<number>(0);
  // Rolling window of (blockNumber, blockTimestamp) for chain-rate calculation.
  // We keep up to 500 samples (~45s at 11 bps) and use two windows:
  //  - 30s of block-time for the displayed bps (very stable)
  //  - 10s of block-time for drain pacing (responsive enough to adapt)
  const blockLogRef = useRef<BlockLog>([]);
  // Cached drain interval in ms, derived from chain block timestamps
  const drainIntervalRef = useRef<number>(90); // initial guess ~11 bps
  const drainOneRef = useRef<() => void>(() => {});

  const scheduleDrain = useCallback((fromBurstStart: boolean) => {
    if (drainTimerRef.current !== null || queueRef.current.length === 0) return;

    const now = performance.now();
    const queue = queueRef.current;
    const interval = getDrainInterval(drainIntervalRef.current, queue.length);
    const sinceLastDrain = lastDrainAtRef.current > 0 ? now - lastDrainAtRef.current : null;

    let delay = 0;
    if (sinceLastDrain !== null) {
      delay = Math.max(0, interval - sinceLastDrain);
    }

    // Keep a small client-side buffer so the UI doesn't spend its final couple
    // of items too early and expose the backend's batch cadence as a pause.
    const oldestBufferedFor = now - queue[0].receivedAt;
    if (queue.length < STREAM_BUFFER_BLOCKS && oldestBufferedFor < MAX_BUFFER_WAIT_MS) {
      delay = Math.max(delay, Math.min(interval, MAX_BUFFER_WAIT_MS - oldestBufferedFor));
    }

    // When a backlog arrives after idle time, add a brief lead-in so the UI
    // starts "dripping" rather than snapping to the first block immediately.
    if (fromBurstStart && queue.length > 1) {
      const burstLeadIn = Math.min(
        MAX_BURST_LEAD_IN_MS,
        interval * Math.min(queue.length - 1, STREAM_BUFFER_BLOCKS + 1),
      );
      delay = Math.max(delay, burstLeadIn);
    }

    drainTimerRef.current = window.setTimeout(() => drainOneRef.current(), delay);
  }, []);

  // Kick the drain loop when new items arrive.
  const kickDrain = useCallback(() => {
    scheduleDrain(true);
  }, [scheduleDrain]);

  const connect = useCallback(function connectSSE() {
    if (esRef.current) {
      esRef.current.close();
    }

    const url = `${API_BASE_URL}/events`;
    const es = new EventSource(url);
    esRef.current = es;

    es.onopen = () => {
      setConnected(true);
      setError(null);
    };

    es.addEventListener('new_block', (e: MessageEvent) => {
      try {
        const data: NewBlockEvent = JSON.parse(e.data);

        // Log block number + on-chain timestamp for true chain-rate calculation
        const log = blockLogRef.current;
        log.push({ num: data.block.number, ts: data.block.timestamp });

        // Keep last 500 samples (~45s at 11 bps)
        if (log.length > 500) {
          blockLogRef.current = log.slice(-500);
        }

        // Displayed bps: 30s window for stability, 5s fallback while bootstrapping
        const displayBps = computeBpsFromLog(blockLogRef.current, 30, 5);
        if (displayBps !== null) setBps(displayBps);

        // Drain pacing: 10s window for moderate responsiveness, 2s fallback
        const drainBps = computeBpsFromLog(blockLogRef.current, 10, 2);
        if (drainBps !== null) {
          drainIntervalRef.current = Math.max(
            MIN_DRAIN_INTERVAL_MS,
            Math.min(MAX_DRAIN_INTERVAL_MS, 1000 / drainBps),
          );
        }

        // Push to ref queue — synchronous, never lost by React batching
        queueRef.current.push({ event: data, receivedAt: performance.now() });
        kickDrain();
      } catch {
        // Ignore malformed events
      }
    });

    es.addEventListener('da_batch', (e: MessageEvent) => {
      try {
        const data: DaBatchEvent = JSON.parse(e.data);
        if (data.updates?.length) {
          setLatestDaUpdate(data.updates[data.updates.length - 1]);
          for (const cb of daSubscribersRef.current) cb(data.updates);
        }
      } catch {
        // Ignore malformed events
      }
    });

    es.onerror = (e) => {
      setConnected(false);
      setError(`SSE ${e.type || 'error'}; retrying`);
      es.close();
      esRef.current = null;

      if (reconnectTimeoutRef.current !== null) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      reconnectTimeoutRef.current = window.setTimeout(() => {
        reconnectTimeoutRef.current = null;
        connectSSE();
      }, 2000);
    };
  }, [kickDrain]);

  // Drain one block from the queue at the chain's natural cadence.
  // Assigned to drainOneRef so scheduleDrain can call it without a circular dependency.
  drainOneRef.current = drainOne;
  function drainOne() {
    const queue = queueRef.current;
    drainTimerRef.current = null;

    if (queue.length === 0) return; // idle — kickDrain will restart when items arrive

    // If queue is backing up (> 50 items), skip to near the end
    if (queue.length > 50) {
      const skip = queue.splice(0, queue.length - 5);
      const lastSkipped = skip[skip.length - 1].event;
      setHeight(lastSkipped.block.number);
    }

    const next = queue.shift()!.event;
    setLatestBlock(next);
    setHeight(next.block.number);
    lastDrainAtRef.current = performance.now();

    // Schedule next drain if more items remain
    if (queue.length > 0) {
      scheduleDrain(false);
    }
  }

  useEffect(() => {
    connect();

    return () => {
      if (esRef.current) {
        esRef.current.close();
        esRef.current = null;
      }
      if (reconnectTimeoutRef.current !== null) {
        clearTimeout(reconnectTimeoutRef.current);
        reconnectTimeoutRef.current = null;
      }
      if (drainTimerRef.current !== null) {
        clearTimeout(drainTimerRef.current);
        drainTimerRef.current = null;
      }
      lastDrainAtRef.current = 0;
    };
  }, [connect]);

  return { latestBlock, latestDaUpdate, height, connected, error, bps, subscribeDa };
}
