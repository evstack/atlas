import { useCallback, useEffect, useRef, useState } from 'react';
import { getStatus } from '../api/status';

export interface LatestHeightState {
  height: number | null;
  loading: boolean;
  error: string | null;
  lastUpdatedAt: number | null;
  bps: number | null;
}

export default function useLatestBlockHeight(pollMs = 2000, _windowBlocks = 1000000): LatestHeightState {
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

  const fetchHeight = useCallback(async () => {
    if (fetchingRef.current) return;
    fetchingRef.current = true;
    try {
      const status = await getStatus();
      const now = Date.now();
      const latestHeight = status?.block_height;
      if (typeof latestHeight === 'number') {
        if (latestHeight !== heightRef.current) {
          // Update height and time of update
          heightRef.current = latestHeight;
          setHeight(latestHeight);
          setLastUpdatedAt(now);
        }
        // Update bps using wall-clock deltas between status samples
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
        setError(null);
      } else {
        setHeight(null);
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to fetch latest height');
    } finally {
      setLoading(false);
      fetchingRef.current = false;
    }
  }, []);

  useEffect(() => {
    fetchHeight();
    const id = setInterval(fetchHeight, pollMs);
    return () => clearInterval(id);
  }, [pollMs, fetchHeight]);

  return { height, loading, error, lastUpdatedAt, bps };
}
