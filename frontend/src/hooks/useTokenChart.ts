import { useEffect, useState } from 'react';
import { getTokenChart, type ChartWindow, type TokenChartPoint } from '../api/chartData';

interface TokenChartData {
  data: TokenChartPoint[];
  loading: boolean;
  error: string | null;
}

export function useTokenChart(address: string | undefined, window: ChartWindow): TokenChartData {
  const [data, setData] = useState<TokenChartPoint[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!address) return;
    let mounted = true;

    setData([]);
    setLoading(true);

    const fetch = async () => {
      try {
        setError(null);
        const points = await getTokenChart(address, window);
        if (mounted) setData(points);
      } catch (err) {
        if (mounted) setError(err instanceof Error ? err.message : 'Failed to load chart data');
      } finally {
        if (mounted) setLoading(false);
      }
    };

    fetch();
    const id = setInterval(fetch, 30_000);
    return () => {
      mounted = false;
      clearInterval(id);
    };
  }, [address, window]);

  return { data, loading, error };
}
