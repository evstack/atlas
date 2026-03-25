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
  dailyTxsLoading: boolean;
  blocksChartLoading: boolean;
  gasPriceLoading: boolean;
  dailyTxsError: string | null;
  blocksChartError: string | null;
  gasPriceError: string | null;
}

export function useChartData(window: ChartWindow): ChartData {
  const [blocksChart, setBlocksChart] = useState<BlockChartPoint[]>([]);
  const [dailyTxs, setDailyTxs] = useState<DailyTxPoint[]>([]);
  const [gasPriceChart, setGasPriceChart] = useState<GasPricePoint[]>([]);
  const [dailyTxsLoading, setDailyTxsLoading] = useState(true);
  const [blocksChartLoading, setBlocksChartLoading] = useState(true);
  const [gasPriceLoading, setGasPriceLoading] = useState(true);
  const [dailyTxsError, setDailyTxsError] = useState<string | null>(null);
  const [blocksChartError, setBlocksChartError] = useState<string | null>(null);
  const [gasPriceError, setGasPriceError] = useState<string | null>(null);

  // Daily txs are window-independent — fetch once and refresh on a slow interval
  useEffect(() => {
    let mounted = true;
    setDailyTxsLoading(true);
    const fetchDaily = async () => {
      try {
        setDailyTxsError(null);
        const daily = await getDailyTxs();
        if (mounted) setDailyTxs(daily);
      } catch (err) {
        if (mounted)
          setDailyTxsError(err instanceof Error ? err.message : 'Failed to load daily transactions');
      } finally {
        if (mounted) setDailyTxsLoading(false);
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
        setBlocksChartError(null);
        const blocks = await getBlocksChart(window);
        if (mounted) setBlocksChart(blocks);
      } catch (err) {
        if (mounted)
          setBlocksChartError(err instanceof Error ? err.message : 'Failed to load blocks chart');
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
        setGasPriceError(null);
        const gasPrice = await getGasPriceChart(window);
        if (mounted) setGasPriceChart(gasPrice);
      } catch (err) {
        if (mounted)
          setGasPriceError(err instanceof Error ? err.message : 'Failed to load gas price chart');
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

  return {
    blocksChart,
    dailyTxs,
    gasPriceChart,
    dailyTxsLoading,
    blocksChartLoading,
    gasPriceLoading,
    dailyTxsError,
    blocksChartError,
    gasPriceError,
  };
}
