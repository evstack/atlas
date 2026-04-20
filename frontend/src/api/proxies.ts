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

interface ProxyInfoResponse {
  is_proxy: boolean;
  proxy: ProxyInfo | null;
}

export async function getContractProxy(address: string): Promise<ProxyInfo | null> {
  try {
    const resp = await client.get<ProxyInfoResponse>(`/contracts/${address}/proxy`);
    return resp.proxy;
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
