import client from "./client";

export interface HeightResponse {
  block_height: number;
  indexed_at?: string; // ISO timestamp, absent when no blocks indexed
}

export interface ChainStatusResponse {
  chain_id: string;
  chain_name: string;
  block_height: number;
  total_transactions: number;
  total_addresses: number;
  indexed_at: string; // ISO timestamp
}

export async function getHeight(): Promise<HeightResponse> {
  return client.get<HeightResponse>("/height");
}

export async function getChainStatus(): Promise<ChainStatusResponse> {
  return client.get<ChainStatusResponse>("/status");
}
