import { useCallback, useEffect, useRef, useState } from 'react';
import type { Block } from '../types';
import { API_BASE_URL } from '../api/client';
import { getStatus } from '../api/status';

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
export type DaResyncSubscriber = () => void;

export interface BlockSSEState {
  latestBlock: NewBlockEvent | null;
  height: number | null;
  connected: boolean;
  error: string | null;
  bps: number | null;
  subscribeDa: (cb: DaSubscriber) => () => void;
  subscribeDaResync: (cb: DaResyncSubscriber) => () => void;
}

type BlockLog = { num: number; ts: number }[];

const MIN_DRAIN_MS = 30;
const MAX_DRAIN_MS = 500;
const POLL_INTERVAL_MS = 2000;

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

export default function useBlockSSE(): BlockSSEState {
  const [latestBlock, setLatestBlock] = useState<NewBlockEvent | null>(null);
  const [height, setHeight] = useState<number | null>(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bps, setBps] = useState<number | null>(null);

  const daSubscribersRef = useRef<Set<DaSubscriber>>(new Set());
  const daResyncSubscribersRef = useRef<Set<DaResyncSubscriber>>(new Set());
  const subscribeDa = useCallback((cb: DaSubscriber) => {
    daSubscribersRef.current.add(cb);
    return () => {
      daSubscribersRef.current.delete(cb);
    };
  }, []);
  const subscribeDaResync = useCallback((cb: DaResyncSubscriber) => {
    daResyncSubscribersRef.current.add(cb);
    return () => {
      daResyncSubscribersRef.current.delete(cb);
    };
  }, []);

  const esRef = useRef<EventSource | null>(null);
  const queueRef = useRef<NewBlockEvent[]>([]);
  const drainTimerRef = useRef<number | null>(null);
  const drainOneRef = useRef<(() => void) | null>(null);
  const drainIntervalRef = useRef<number>(90); // initial guess ~11 bps
  const blockLogRef = useRef<BlockLog>([]);
  const pollTimerRef = useRef<number | null>(null);
  const connectedRef = useRef(false);
  const highestSeenRef = useRef<number>(-1);

  const scheduleDrain = useCallback(() => {
    if (drainTimerRef.current !== null || queueRef.current.length === 0) return;
    const interval = Math.max(MIN_DRAIN_MS, drainIntervalRef.current);
    drainTimerRef.current = window.setTimeout(() => drainOneRef.current?.(), interval);
  }, []);

  const drainOne = useCallback(() => {
    drainTimerRef.current = null;
    const queue = queueRef.current;
    if (queue.length === 0) return;

    if (queue.length > 50) {
      const skip = queue.splice(0, queue.length - 5);
      const last = skip[skip.length - 1];
      setHeight(last.block.number);
    }

    const next = queue.shift()!;
    setLatestBlock(next);
    setHeight(next.block.number);

    if (queue.length > 0) scheduleDrain();
  }, [scheduleDrain]);

  useEffect(() => {
    drainOneRef.current = drainOne;
  }, [drainOne]);

  const enqueue = useCallback((data: NewBlockEvent) => {
    if (data.block.number <= highestSeenRef.current) return;
    highestSeenRef.current = data.block.number;

    const log = blockLogRef.current;
    log.push({ num: data.block.number, ts: data.block.timestamp });
    if (log.length > 500) blockLogRef.current = log.slice(-500);

    const displayBps = computeBpsFromLog(blockLogRef.current, 30, 5);
    if (displayBps !== null) setBps(displayBps);

    const drainBps = computeBpsFromLog(blockLogRef.current, 10, 2);
    if (drainBps !== null) {
      drainIntervalRef.current = Math.max(MIN_DRAIN_MS, Math.min(MAX_DRAIN_MS, 1000 / drainBps));
    }

    queueRef.current.push(data);
    scheduleDrain();
  }, [scheduleDrain]);

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
        if (typeof status?.block_height === 'number' && !connectedRef.current && status.block_height > highestSeenRef.current) {
          highestSeenRef.current = status.block_height;
          setHeight(status.block_height);
        }
      } catch {
        // ignore polling errors
      }
    };
    void poll();
    pollTimerRef.current = window.setInterval(() => {
      void poll();
    }, POLL_INTERVAL_MS);
  }, []);

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

    es.addEventListener('da_batch', (e: MessageEvent) => {
      try {
        const data: DaBatchEvent = JSON.parse(e.data);
        if (data.updates?.length) {
          for (const cb of daSubscribersRef.current) cb(data.updates);
        }
      } catch {
        // ignore malformed events
      }
    });

    es.addEventListener('da_resync', () => {
      for (const cb of daResyncSubscribersRef.current) cb();
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

  return { latestBlock, height, connected, error, bps, subscribeDa, subscribeDaResync };
}
