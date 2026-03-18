import client from './client';

export interface StatusResponse {
  block_height: number;
  indexed_at?: string;
}

export async function getStatus(): Promise<StatusResponse> {
  return client.get<StatusResponse>('/status');
}
