import { useCallback, useEffect, useRef, useState } from 'react';
import type { Block } from '../types';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || 'http://localhost:3000/api';

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
  const blockLogRef = useRef<{ num: number; ts: number }[]>([]);
  // Cached drain interval in ms, derived from chain block timestamps
  const drainIntervalRef = useRef<number>(90); // initial guess ~11 bps

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
        blockLogRef.current.push({
          num: data.block.number,
          ts: data.block.timestamp, // unix seconds from the chain
        });

        // Keep last 500 samples (~45s at 11 bps)
        if (blockLogRef.current.length > 500) {
          blockLogRef.current = blockLogRef.current.slice(-500);
        }

        // Recalculate bps and drain interval from chain timestamps.
        const log = blockLogRef.current;
        const newest = log[log.length - 1];

        // Displayed bps: use 30s window for maximum stability
        for (let i = 0; i < log.length - 1; i++) {
          const span = newest.ts - log[i].ts;
          if (span >= 30) {
            const chainBps = (newest.num - log[i].num) / span;
            setBps(chainBps);
            break;
          }
          // If we don't have 30s yet, use whatever we have (≥5s)
          if (i === 0 && span >= 5) {
            const chainBps = (newest.num - log[0].num) / span;
            setBps(chainBps);
          }
        }

        // Drain pacing: use 10s window for moderate responsiveness
        for (let i = 0; i < log.length - 1; i++) {
          const span = newest.ts - log[i].ts;
          if (span >= 10) {
            const drainBps = (newest.num - log[i].num) / span;
            drainIntervalRef.current = Math.max(30, Math.min(500, 1000 / drainBps));
            break;
          }
          // Bootstrap: use ≥2s while building up
          if (i === 0 && span >= 2) {
            const drainBps = (newest.num - log[0].num) / span;
            drainIntervalRef.current = Math.max(30, Math.min(500, 1000 / drainBps));
          }
        }

        // Push to ref queue — synchronous, never lost by React batching
        queueRef.current.push(data);
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
  }, []);

  // Drain loop: emit one block per tick at the chain's natural cadence.
  useEffect(() => {
    let running = true;

    const drain = () => {
      if (!running) return;
      const queue = queueRef.current;

      if (queue.length === 0) {
        // Nothing to drain — check again soon
        drainTimerRef.current = window.setTimeout(drain, 30);
        return;
      }

      // If queue is backing up (> 50 items), skip to near the end
      if (queue.length > 50) {
        const skip = queue.splice(0, queue.length - 5);
        const lastSkipped = skip[skip.length - 1];
        setHeight(lastSkipped.block.number);
      }

      const next = queue.shift()!;
      setLatestBlock(next);
      setHeight(next.block.number);

      // Use the chain-rate interval, but if queue is growing speed up gently
      let interval = drainIntervalRef.current;
      if (queue.length > 5) {
        interval = interval * 0.7;
      }
      drainTimerRef.current = window.setTimeout(drain, interval);
    };

    drainTimerRef.current = window.setTimeout(drain, 30);

    return () => {
      running = false;
      if (drainTimerRef.current !== null) {
        clearTimeout(drainTimerRef.current);
        drainTimerRef.current = null;
      }
    };
  }, []);

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
