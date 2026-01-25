import { useState, useEffect, useCallback } from 'react';
import type { Token, TokenHolder, TokenTransfer, AddressTokenBalance, ApiError } from '../types';
import {
  getTokens,
  getToken,
  getTokenHolders,
  getTokenTransfers,
  getAddressTokens,
} from '../api/tokens';
import type {
  GetTokensParams,
  GetTokenHoldersParams,
  GetTokenTransfersParams,
  GetAddressTokensParams,
} from '../api/tokens';

interface UseTokensResult {
  tokens: Token[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useTokens(params: GetTokensParams = {}): UseTokensResult {
  const [tokens, setTokens] = useState<Token[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTokens = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getTokens(params);
      setTokens(response.data);
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
    fetchTokens();
  }, [fetchTokens]);

  return { tokens, pagination, loading, error, refetch: fetchTokens };
}

interface UseTokenResult {
  token: Token | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useToken(address: string | undefined): UseTokenResult {
  const [token, setToken] = useState<Token | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchToken = useCallback(async () => {
    if (!address) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getToken(address);
      setToken(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [address]);

  useEffect(() => {
    fetchToken();
  }, [fetchToken]);

  return { token, loading, error, refetch: fetchToken };
}

interface UseTokenHoldersResult {
  holders: TokenHolder[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useTokenHolders(address: string | undefined, params: GetTokenHoldersParams = {}): UseTokenHoldersResult {
  const [holders, setHolders] = useState<TokenHolder[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchHolders = useCallback(async () => {
    if (!address) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getTokenHolders(address, params);
      setHolders(response.data);
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
  }, [address, params.page, params.limit]);

  useEffect(() => {
    fetchHolders();
  }, [fetchHolders]);

  return { holders, pagination, loading, error, refetch: fetchHolders };
}

interface UseTokenTransfersResult {
  transfers: TokenTransfer[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useTokenTransfers(address: string | undefined, params: GetTokenTransfersParams = {}): UseTokenTransfersResult {
  const [transfers, setTransfers] = useState<TokenTransfer[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTransfers = useCallback(async () => {
    if (!address) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getTokenTransfers(address, params);
      setTransfers(response.data);
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
  }, [address, params.page, params.limit]);

  useEffect(() => {
    fetchTransfers();
  }, [fetchTransfers]);

  return { transfers, pagination, loading, error, refetch: fetchTransfers };
}

interface UseAddressTokensResult {
  balances: AddressTokenBalance[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useAddressTokens(address: string | undefined, params: GetAddressTokensParams = {}): UseAddressTokensResult {
  const [balances, setBalances] = useState<AddressTokenBalance[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchBalances = useCallback(async () => {
    if (!address) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getAddressTokens(address, params);
      setBalances(response.data);
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
  }, [address, params.page, params.limit]);

  useEffect(() => {
    fetchBalances();
  }, [fetchBalances]);

  return { balances, pagination, loading, error, refetch: fetchBalances };
}
