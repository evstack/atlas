import client from './client';

export interface HeightResponse {
  block_height: number;
  indexed_at: string; // ISO timestamp
}

export interface ChainStatusResponse {
  chain_id: number;
  chain_name: string;
  block_height: number;
  total_transactions: number;
  total_addresses: number;
  indexed_at: string; // ISO timestamp
}

export async function getStatus(): Promise<HeightResponse> {
  const response = await client.get<HeightResponse>('/height');
  return response.data;
}

export async function getChainStatus(): Promise<ChainStatusResponse> {
  const response = await client.get<ChainStatusResponse>('/status');
  return response.data;
}
