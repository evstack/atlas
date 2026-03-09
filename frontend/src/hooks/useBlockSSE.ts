import { useCallback, useEffect, useRef, useState } from 'react';
import type { Block } from '../types';
import { API_BASE_URL } from '../api/client';

export interface NewBlockEvent {
  block: Block;
}

export interface BlockSSEState {
  latestBlock: NewBlockEvent | null;
  height: number | null;
  connected: boolean;
  error: string | null;
  bps: number | null;
}

type BlockLog = { num: number; ts: number }[];

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
 * Connects to the SSE endpoint and delivers block events one-by-one.
 *
 * SSE events arrive in bursts (the backend polls every 200ms and yields all
 * new blocks at once). React batching would collapse rapid setState calls,
 * so we buffer into a ref-based queue and drain one at a time.
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
  const [height, setHeight] = useState<number | null>(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bps, setBps] = useState<number | null>(null);
  const esRef = useRef<EventSource | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const queueRef = useRef<NewBlockEvent[]>([]);
  const drainTimerRef = useRef<number | null>(null);
  // Rolling window of (blockNumber, blockTimestamp) for chain-rate calculation.
  // We keep up to 500 samples (~45s at 11 bps) and use two windows:
  //  - 30s of block-time for the displayed bps (very stable)
  //  - 10s of block-time for drain pacing (responsive enough to adapt)
  const blockLogRef = useRef<BlockLog>([]);
  // Cached drain interval in ms, derived from chain block timestamps
  const drainIntervalRef = useRef<number>(90); // initial guess ~11 bps

  // Kick the drain loop when new items arrive (avoids 33hz idle polling)
  const kickDrain = useCallback(() => {
    // If drain is already scheduled, let it run naturally
    if (drainTimerRef.current !== null) return;
    drainTimerRef.current = window.setTimeout(drainOne, 0);
  }, []);

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
          drainIntervalRef.current = Math.max(30, Math.min(500, 1000 / drainBps));
        }

        // Push to ref queue — synchronous, never lost by React batching
        queueRef.current.push(data);
        kickDrain();
      } catch {
        // Ignore malformed events
      }
    });

    es.onerror = () => {
      setConnected(false);
      es.close();
      esRef.current = null;

      reconnectTimeoutRef.current = window.setTimeout(() => {
        reconnectTimeoutRef.current = null;
        connectSSE();
      }, 2000);
    };
  }, [kickDrain]);

  // Drain one block from the queue at the chain's natural cadence.
  function drainOne() {
    const queue = queueRef.current;
    drainTimerRef.current = null;

    if (queue.length === 0) return; // idle — kickDrain will restart when items arrive

    // If queue is backing up (> 50 items), skip to near the end
    if (queue.length > 50) {
      const skip = queue.splice(0, queue.length - 5);
      const lastSkipped = skip[skip.length - 1];
      setHeight(lastSkipped.block.number);
    }

    const next = queue.shift()!;
    setLatestBlock(next);
    setHeight(next.block.number);

    // Schedule next drain if more items remain
    if (queue.length > 0) {
      let interval = drainIntervalRef.current;
      if (queue.length > 5) interval = interval * 0.7;
      drainTimerRef.current = window.setTimeout(drainOne, interval);
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
    };
  }, [connect]);

  return { latestBlock, height, connected, error, bps };
}
