import { useCallback, useEffect, useMemo, useState } from 'react';
import { getTotals, getDailyTxCount, type Totals } from '../api/stats';
import useLatestBlockHeight from './useLatestBlockHeight';

export default function useStats() {
  const [totals, setTotals] = useState<Totals | null>(null);
  const [dailyTx, setDailyTx] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { bps } = useLatestBlockHeight(4000, 1_000_000);

  const fetchAll = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [t, d] = await Promise.all([getTotals(), getDailyTxCount(30, 100)]);
      setTotals(t);
      setDailyTx(d);
    } catch (e: any) {
      setError(e?.error || e?.message || 'Failed to load stats');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetchAll(); }, [fetchAll]);

  const avgBlockTimeSec = useMemo(() => {
    if (!bps || bps <= 0) return null;
    return 1 / bps;
  }, [bps]);

  return { totals, dailyTx, avgBlockTimeSec, loading, error, refetch: fetchAll };
}

