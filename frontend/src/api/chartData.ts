import client from './client';

export type ChartWindow = '1h' | '6h' | '24h' | '7d' | '1m' | '6m' | '1y';

export interface BlockChartPoint {
  bucket: string; // ISO timestamp
  tx_count: number;
  avg_gas_used: number; // average gas used per block in this bucket
}

export interface DailyTxPoint {
  day: string; // YYYY-MM-DD
  tx_count: number;
}

export interface GasPricePoint {
  bucket: string; // ISO timestamp
  avg_gas_price: number; // wei
}

export function getBlocksChart(window: ChartWindow): Promise<BlockChartPoint[]> {
  return client.get<BlockChartPoint[]>('/stats/blocks-chart', { params: { window } });
}

export function getDailyTxs(): Promise<DailyTxPoint[]> {
  return client.get<DailyTxPoint[]>('/stats/daily-txs');
}

export function getGasPriceChart(window: ChartWindow): Promise<GasPricePoint[]> {
  return client.get<GasPricePoint[]>('/stats/gas-price', { params: { window } });
}

export interface TokenChartPoint {
  bucket: string; // ISO timestamp
  transfer_count: number;
  volume: number; // in human-readable token units (divided by 10^decimals on backend)
}

export function getTokenChart(address: string, window: ChartWindow): Promise<TokenChartPoint[]> {
  return client.get<TokenChartPoint[]>(`/tokens/${address}/chart`, { params: { window } });
}
