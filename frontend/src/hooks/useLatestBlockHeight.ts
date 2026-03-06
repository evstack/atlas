import { useCallback, useEffect, useRef, useState } from 'react';
import { getStatus } from '../api/status';

export interface LatestHeightState {
  height: number | null;
  loading: boolean;
  error: string | null;
  lastUpdatedAt: number | null;
  bps: number | null;
}

/**
 * Tracks the latest block height and computes blocks-per-second (bps).
 * When SSE is connected, uses sseBps (derived from on-chain block timestamps)
 * for a stable block-time display. Falls back to wall-clock EMA when polling.
 */
export default function useLatestBlockHeight(
  pollMs = 2000,
  _windowBlocks = 1000000,
  sseHeight: number | null = null,
  sseConnected = false,
  sseBps: number | null = null,
): LatestHeightState {
  void _windowBlocks;
  const [height, setHeight] = useState<number | null>(null);
  const heightRef = useRef<number | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdatedAt, setLastUpdatedAt] = useState<number | null>(null);
  const fetchingRef = useRef(false);
  const [bps, setBps] = useState<number | null>(null);
  const prevSampleRef = useRef<{ h: number; t: number } | null>(null);
  const alphaRef = useRef<number>(0.25); // smoothing factor for EMA

  // When SSE provides bps from block timestamps, use it directly
  useEffect(() => {
    if (sseConnected && sseBps != null) {
      setBps(sseBps);
    }
  }, [sseConnected, sseBps]);

  // Process a new height value (from either SSE or polling)
  const processHeight = useCallback((latestHeight: number, fromSSE: boolean) => {
    const now = Date.now();
    if (latestHeight !== heightRef.current) {
      heightRef.current = latestHeight;
      setHeight(latestHeight);
      setLastUpdatedAt(now);
    }
    // Only compute wall-clock EMA bps when polling (not SSE)
    if (!fromSSE) {
      const prev = prevSampleRef.current;
      const curr = { h: latestHeight, t: now };
      if (prev && curr.t > prev.t && curr.h >= prev.h) {
        const dh = curr.h - prev.h;
        const dt = (curr.t - prev.t) / 1000;
        const inst = dt > 0 ? dh / dt : 0;
        const alpha = alphaRef.current;
        setBps((prevBps) => (prevBps == null ? inst : prevBps + alpha * (inst - prevBps)));
      }
      prevSampleRef.current = curr;
    }
    setError(null);
    setLoading(false);
  }, []);

  // Handle SSE height updates
  useEffect(() => {
    if (sseHeight != null) {
      processHeight(sseHeight, true);
    }
  }, [sseHeight, processHeight]);

  // HTTP polling fallback — only active when SSE is not connected
  const fetchHeight = useCallback(async () => {
    if (fetchingRef.current) return;
    fetchingRef.current = true;
    try {
      const status = await getStatus();
      const latestHeight = status?.block_height;
      if (typeof latestHeight === 'number') {
        processHeight(latestHeight, false);
      } else {
        setHeight(null);
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to fetch latest height');
    } finally {
      setLoading(false);
      fetchingRef.current = false;
    }
  }, [processHeight]);

  useEffect(() => {
    // Skip polling when SSE is connected
    if (sseConnected) return;

    fetchHeight();
    const id = setInterval(fetchHeight, pollMs);
    return () => clearInterval(id);
  }, [pollMs, fetchHeight, sseConnected]);

  return { height, loading, error, lastUpdatedAt, bps };
}
