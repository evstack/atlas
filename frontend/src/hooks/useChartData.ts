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

function fillMissingGasPriceBuckets(points: GasPricePoint[]): GasPricePoint[] {
  let lastObservedPrice: number | null = null;

  return points.map((point) => {
    if (point.avg_gas_price !== null) {
      lastObservedPrice = point.avg_gas_price;
      return point;
    }

    return {
      ...point,
      avg_gas_price: lastObservedPrice,
    };
  });
}

function getChartErrorMessage(err: unknown, fallback: string): string {
  if (err && typeof err === 'object' && 'error' in err && typeof (err as { error: unknown }).error === 'string') {
    return (err as { error: string }).error;
  }

  if (err instanceof Error) {
    return err.message;
  }

  return fallback;
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
    let timeoutId: number | undefined;
    setDailyTxsLoading(true);

    const fetchDaily = async () => {
      try {
        setDailyTxsError(null);
        const daily = await getDailyTxs();
        if (mounted) setDailyTxs(daily);
      } catch (err) {
        if (mounted) {
          setDailyTxsError(getChartErrorMessage(err, 'Failed to load daily transactions'));
        }
      } finally {
        if (mounted) setDailyTxsLoading(false);
      }
      if (mounted) {
        timeoutId = globalThis.setTimeout(() => {
          void fetchDaily();
        }, 60_000);
      }
    };

    void fetchDaily();

    return () => {
      mounted = false;
      if (timeoutId !== undefined) {
        clearTimeout(timeoutId);
      }
    };
  }, []);

  // Blocks chart depends on the selected window
  useEffect(() => {
    let mounted = true;
    let timeoutId: number | undefined;
    setBlocksChartLoading(true);

    const fetch = async () => {
      try {
        setBlocksChartError(null);
        const blocks = await getBlocksChart(window);
        if (mounted) setBlocksChart(blocks);
      } catch (err) {
        if (mounted) {
          setBlocksChartError(getChartErrorMessage(err, 'Failed to load blocks chart'));
        }
      } finally {
        if (mounted) setBlocksChartLoading(false);
      }
      if (mounted) {
        timeoutId = globalThis.setTimeout(() => {
          void fetch();
        }, 30_000);
      }
    };

    void fetch();

    return () => {
      mounted = false;
      if (timeoutId !== undefined) {
        clearTimeout(timeoutId);
      }
    };
  }, [window]);

  // Gas price chart depends on the selected window
  useEffect(() => {
    let mounted = true;
    let timeoutId: number | undefined;
    setGasPriceLoading(true);

    const fetch = async () => {
      try {
        setGasPriceError(null);
        const gasPrice = await getGasPriceChart(window);
        if (mounted) setGasPriceChart(fillMissingGasPriceBuckets(gasPrice));
      } catch (err) {
        if (mounted) {
          setGasPriceError(getChartErrorMessage(err, 'Failed to load gas price chart'));
        }
      } finally {
        if (mounted) setGasPriceLoading(false);
      }
      if (mounted) {
        timeoutId = globalThis.setTimeout(() => {
          void fetch();
        }, 30_000);
      }
    };

    void fetch();

    return () => {
      mounted = false;
      if (timeoutId !== undefined) {
        clearTimeout(timeoutId);
      }
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
