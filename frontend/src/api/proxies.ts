import client from './client';
import type { ProxyInfo, CombinedAbi, PaginatedResponse } from '../types';

export interface GetProxiesParams {
  page?: number;
  limit?: number;
}

export async function getProxies(params: GetProxiesParams = {}): Promise<PaginatedResponse<ProxyInfo>> {
  const { page = 1, limit = 20 } = params;
  return client.get<PaginatedResponse<ProxyInfo>>('/proxies', { params: { page, limit } });
}

export async function getContractProxy(address: string): Promise<ProxyInfo | null> {
  try {
    return await client.get<ProxyInfo>(`/contracts/${address}/proxy`);
  } catch {
    return null;
  }
}

export async function getContractCombinedAbi(address: string): Promise<CombinedAbi | null> {
  try {
    return await client.get<CombinedAbi>(`/contracts/${address}/combined-abi`);
  } catch {
    return null;
  }
}
