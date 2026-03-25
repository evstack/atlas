import { useEffect, useState } from 'react';
import {
  getBlocksChart,
  getDailyTxs,
  getGasPriceChart,
  type BlockChartPoint,
  type ChartWindow,
  type DailyTxPoint,
  type GasPricePoint,
} from '../api/chartData';

interface ChartData {
  blocksChart: BlockChartPoint[];
  dailyTxs: DailyTxPoint[];
  gasPriceChart: GasPricePoint[];
  blocksChartLoading: boolean;
  gasPriceLoading: boolean;
  error: string | null;
}

export function useChartData(window: ChartWindow): ChartData {
  const [blocksChart, setBlocksChart] = useState<BlockChartPoint[]>([]);
  const [dailyTxs, setDailyTxs] = useState<DailyTxPoint[]>([]);
  const [gasPriceChart, setGasPriceChart] = useState<GasPricePoint[]>([]);
  const [blocksChartLoading, setBlocksChartLoading] = useState(true);
  const [gasPriceLoading, setGasPriceLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Daily txs are window-independent — fetch once and refresh on a slow interval
  useEffect(() => {
    let mounted = true;
    const fetchDaily = async () => {
      try {
        const daily = await getDailyTxs();
        if (mounted) setDailyTxs(daily);
      } catch {
        // non-critical, ignore
      }
    };
    fetchDaily();
    const id = setInterval(fetchDaily, 60_000);
    return () => {
      mounted = false;
      clearInterval(id);
    };
  }, []);

  // Blocks chart depends on the selected window
  useEffect(() => {
    let mounted = true;
    setBlocksChartLoading(true);
    const fetch = async () => {
      try {
        setError(null);
        const blocks = await getBlocksChart(window);
        if (mounted) setBlocksChart(blocks);
      } catch (err) {
        if (mounted) setError(err instanceof Error ? err.message : 'Failed to load chart data');
      } finally {
        if (mounted) setBlocksChartLoading(false);
      }
    };
    fetch();
    const id = setInterval(fetch, 30_000);
    return () => {
      mounted = false;
      clearInterval(id);
    };
  }, [window]);

  // Gas price chart depends on the selected window
  useEffect(() => {
    let mounted = true;
    setGasPriceLoading(true);
    const fetch = async () => {
      try {
        const gasPrice = await getGasPriceChart(window);
        if (mounted) setGasPriceChart(gasPrice);
      } catch {
        // non-critical, ignore
      } finally {
        if (mounted) setGasPriceLoading(false);
      }
    };
    fetch();
    const id = setInterval(fetch, 30_000);
    return () => {
      mounted = false;
      clearInterval(id);
    };
  }, [window]);

  return { blocksChart, dailyTxs, gasPriceChart, blocksChartLoading, gasPriceLoading, error };
}
