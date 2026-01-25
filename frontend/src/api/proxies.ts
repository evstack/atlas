import client from './client';
import type { ProxyInfo, CombinedAbi, PaginatedResponse } from '../types';

export interface GetProxiesParams {
  page?: number;
  limit?: number;
}

export async function getProxies(params: GetProxiesParams = {}): Promise<PaginatedResponse<ProxyInfo>> {
  const { page = 1, limit = 20 } = params;
  const response = await client.get<PaginatedResponse<ProxyInfo>>('/proxies', {
    params: { page, limit },
  });
  return response.data;
}

export async function getContractProxy(address: string): Promise<ProxyInfo | null> {
  try {
    const response = await client.get<ProxyInfo>(`/contracts/${address}/proxy`);
    return response.data;
  } catch {
    // Return null if no proxy info exists
    return null;
  }
}

export async function getContractCombinedAbi(address: string): Promise<CombinedAbi | null> {
  try {
    const response = await client.get<CombinedAbi>(`/contracts/${address}/combined-abi`);
    return response.data;
  } catch {
    // Return null if no combined ABI exists
    return null;
  }
}
