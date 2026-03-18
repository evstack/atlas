import client from './client';
import type { FaucetInfo, FaucetRequestResponse } from '../types';

export async function getFaucetInfo(): Promise<FaucetInfo> {
  const response = await client.get<FaucetInfo>('/faucet/info');
  return response.data;
}

export async function requestFaucet(address: string): Promise<FaucetRequestResponse> {
  const response = await client.post<FaucetRequestResponse>('/faucet', { address });
  return response.data;
}
