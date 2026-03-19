import client from './client';
import type { FaucetInfo, FaucetRequestResponse } from '../types';

export async function getFaucetInfo(): Promise<FaucetInfo> {
  return client.get<FaucetInfo>('/faucet/info');
}

export async function requestFaucet(address: string): Promise<FaucetRequestResponse> {
  return client.post<FaucetRequestResponse>('/faucet', { address });
}
