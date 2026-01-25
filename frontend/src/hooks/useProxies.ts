import { useState, useEffect, useCallback } from 'react';
import type { ProxyInfo, CombinedAbi, ApiError } from '../types';
import { getProxies, getContractProxy, getContractCombinedAbi } from '../api/proxies';
import type { GetProxiesParams } from '../api/proxies';

interface UseProxiesResult {
  proxies: ProxyInfo[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useProxies(params: GetProxiesParams = {}): UseProxiesResult {
  const [proxies, setProxies] = useState<ProxyInfo[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchProxies = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getProxies(params);
      setProxies(response.data);
      setPagination({
        page: response.page,
        limit: response.limit,
        total: response.total,
        total_pages: response.total_pages,
      });
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [params.page, params.limit]);

  useEffect(() => {
    fetchProxies();
  }, [fetchProxies]);

  return { proxies, pagination, loading, error, refetch: fetchProxies };
}

interface UseContractProxyResult {
  proxyInfo: ProxyInfo | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useContractProxy(address: string | undefined): UseContractProxyResult {
  const [proxyInfo, setProxyInfo] = useState<ProxyInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchProxyInfo = useCallback(async () => {
    if (!address) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getContractProxy(address);
      setProxyInfo(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [address]);

  useEffect(() => {
    fetchProxyInfo();
  }, [fetchProxyInfo]);

  return { proxyInfo, loading, error, refetch: fetchProxyInfo };
}

interface UseCombinedAbiResult {
  combinedAbi: CombinedAbi | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useCombinedAbi(address: string | undefined): UseCombinedAbiResult {
  const [combinedAbi, setCombinedAbi] = useState<CombinedAbi | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchCombinedAbi = useCallback(async () => {
    if (!address) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getContractCombinedAbi(address);
      setCombinedAbi(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [address]);

  useEffect(() => {
    fetchCombinedAbi();
  }, [fetchCombinedAbi]);

  return { combinedAbi, loading, error, refetch: fetchCombinedAbi };
}
