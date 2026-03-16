import { useCallback, useEffect, useRef, useState } from 'react';
import type { Block } from '../types';
import { API_BASE_URL } from '../api/client';
import { getStatus } from '../api/status';

export interface NewBlockEvent {
  block: Block;
}

export interface BlockSSEState {
  latestBlock: NewBlockEvent | null;
  height: number | null;
  connected: boolean;
  error: string | null;
  bps: number | null;
  lastUpdatedAt: number | null;
}

type BlockLog = { num: number; ts: number }[];

const MIN_DRAIN_MS = 30;
const MAX_DRAIN_MS = 500;
const POLL_INTERVAL_MS = 2000;

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

/**
 * Connects to the SSE endpoint and delivers block events at the chain's natural
 * cadence. Falls back to polling /api/status when SSE disconnects.
 *
 * The drain queue rate-limits setState calls so React doesn't collapse rapid
 * arrivals from backend batch commits. The interval is derived from on-chain
 * block timestamps (not arrival times), making visual cadence match the chain.
 */
export default function useBlockSSE(): BlockSSEState {
  const [latestBlock, setLatestBlock] = useState<NewBlockEvent | null>(null);
  const [height, setHeight] = useState<number | null>(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bps, setBps] = useState<number | null>(null);
  const [lastUpdatedAt, setLastUpdatedAt] = useState<number | null>(null);

  const esRef = useRef<EventSource | null>(null);
  const queueRef = useRef<NewBlockEvent[]>([]);
  const drainTimerRef = useRef<number | null>(null);
  const drainOneRef = useRef<(() => void) | null>(null);
  const drainIntervalRef = useRef<number>(90); // initial guess ~11 bps
  const blockLogRef = useRef<BlockLog>([]);
  const pollTimerRef = useRef<number | null>(null);
  const connectedRef = useRef(false);

  // --- Drain: emit one block per chain-rate interval ---

  const scheduleDrain = useCallback(() => {
    if (drainTimerRef.current !== null || queueRef.current.length === 0) return;
    const interval = Math.max(MIN_DRAIN_MS, drainIntervalRef.current);
    drainTimerRef.current = window.setTimeout(() => drainOneRef.current?.(), interval);
  }, []);

  const drainOne = useCallback(() => {
    drainTimerRef.current = null;
    const queue = queueRef.current;
    if (queue.length === 0) return;

    // If queue backs up, skip to near the end
    if (queue.length > 50) {
      const skip = queue.splice(0, queue.length - 5);
      const last = skip[skip.length - 1];
      setHeight(last.block.number);
    }

    const next = queue.shift()!;
    setLatestBlock(next);
    setHeight(next.block.number);
    setLastUpdatedAt(Date.now());

    if (queue.length > 0) scheduleDrain();
  }, [scheduleDrain]);

  useEffect(() => {
    drainOneRef.current = drainOne;
  }, [drainOne]);

  const enqueue = useCallback((data: NewBlockEvent) => {
    // Update bps from on-chain block timestamps
    const log = blockLogRef.current;
    log.push({ num: data.block.number, ts: data.block.timestamp });
    if (log.length > 500) blockLogRef.current = log.slice(-500);

    // Displayed bps: 30s window for stability, 5s fallback while bootstrapping
    const displayBps = computeBpsFromLog(blockLogRef.current, 30, 5);
    if (displayBps !== null) setBps(displayBps);

    // Drain pacing: 10s window for moderate responsiveness, 2s fallback
    const drainBps = computeBpsFromLog(blockLogRef.current, 10, 2);
    if (drainBps !== null) {
      drainIntervalRef.current = Math.max(MIN_DRAIN_MS, Math.min(MAX_DRAIN_MS, 1000 / drainBps));
    }

    queueRef.current.push(data);
    scheduleDrain();
  }, [scheduleDrain]);

  // --- Polling fallback ---

  const stopPolling = useCallback(() => {
    if (pollTimerRef.current !== null) {
      clearInterval(pollTimerRef.current);
      pollTimerRef.current = null;
    }
  }, []);

  const startPolling = useCallback(() => {
    if (pollTimerRef.current !== null) return;

    const poll = async () => {
      try {
        const status = await getStatus();
        if (typeof status?.block_height === 'number' && !connectedRef.current) {
          setHeight(status.block_height);
          setLastUpdatedAt(Date.now());
        }
      } catch {
        // ignore polling errors
      }
    };
    poll();
    pollTimerRef.current = window.setInterval(poll, POLL_INTERVAL_MS);
  }, []);

  // --- SSE connection ---

  const connect = useCallback(() => {
    if (esRef.current) esRef.current.close();

    const es = new EventSource(`${API_BASE_URL}/events`);
    esRef.current = es;

    es.onopen = () => {
      connectedRef.current = true;
      setConnected(true);
      setError(null);
      stopPolling();
    };

    es.addEventListener('new_block', (e: MessageEvent) => {
      try {
        enqueue(JSON.parse(e.data));
      } catch {
        // ignore malformed events
      }
    });

    es.onerror = (e) => {
      connectedRef.current = false;
      setConnected(false);
      setError(`SSE ${e.type || 'error'}; reconnecting`);
      startPolling();
    };
  }, [enqueue, stopPolling, startPolling]);

  useEffect(() => {
    connect();
    return () => {
      if (esRef.current) {
        esRef.current.close();
        esRef.current = null;
      }
      stopPolling();
      if (drainTimerRef.current !== null) {
        clearTimeout(drainTimerRef.current);
        drainTimerRef.current = null;
      }
    };
  }, [connect, stopPolling]);

  return { latestBlock, height, connected, error, bps, lastUpdatedAt };
}
