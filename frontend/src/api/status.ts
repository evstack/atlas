import client from './client';
import type { ChainFeatures } from '../types';

export interface StatusResponse {
  block_height: number;
  indexed_at?: string; // ISO timestamp, absent when no blocks indexed
  features: ChainFeatures;
}

export async function getStatus(): Promise<StatusResponse> {
  const response = await client.get<StatusResponse>('/status');
  return response.data;
}
