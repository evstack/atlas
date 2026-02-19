import client from './client';

export interface StatusResponse {
  block_height: number;
  indexed_at: string; // ISO timestamp
}

export async function getStatus(): Promise<StatusResponse> {
  const response = await client.get<StatusResponse>('/status');
  return response.data;
}

