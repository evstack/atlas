import { useState, useEffect, useCallback } from 'react';
import type { NftContract, NftToken, ApiError } from '../types';
import { getNftContracts, getNftContract, getNftTokens, getNftToken, getAddressNfts, getNftTransfers, getNftTokenTransfers } from '../api/nfts';
import type { GetNftContractsParams, GetNftTokensParams, GetAddressNftsParams, GetNftTransfersParams } from '../api/nfts';

interface UseNftContractsResult {
  contracts: NftContract[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useNftContracts(params: GetNftContractsParams = {}): UseNftContractsResult {
  const [contracts, setContracts] = useState<NftContract[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchContracts = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getNftContracts(params);
      setContracts(response.data);
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
    fetchContracts();
  }, [fetchContracts]);

  return { contracts, pagination, loading, error, refetch: fetchContracts };
}

interface UseNftContractResult {
  contract: NftContract | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useNftContract(contractAddress: string | undefined): UseNftContractResult {
  const [contract, setContract] = useState<NftContract | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchContract = useCallback(async () => {
    if (!contractAddress) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getNftContract(contractAddress);
      setContract(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [contractAddress]);

  useEffect(() => {
    fetchContract();
  }, [fetchContract]);

  return { contract, loading, error, refetch: fetchContract };
}

interface UseNftTokensResult {
  tokens: NftToken[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useNftTokens(contractAddress: string | undefined, params: GetNftTokensParams = {}): UseNftTokensResult {
  const [tokens, setTokens] = useState<NftToken[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTokens = useCallback(async () => {
    if (!contractAddress) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getNftTokens(contractAddress, params);
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
  }, [contractAddress, params.page, params.limit, params.owner]);

  useEffect(() => {
    fetchTokens();
  }, [fetchTokens]);

  return { tokens, pagination, loading, error, refetch: fetchTokens };
}

interface UseNftTokenResult {
  token: NftToken | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useNftToken(contractAddress: string | undefined, tokenId: string | undefined): UseNftTokenResult {
  const [token, setToken] = useState<NftToken | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchToken = useCallback(async () => {
    if (!contractAddress || !tokenId) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getNftToken(contractAddress, tokenId);
      setToken(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [contractAddress, tokenId]);

  useEffect(() => {
    fetchToken();
  }, [fetchToken]);

  return { token, loading, error, refetch: fetchToken };
}

interface UseNftTokenTransfersResult {
  transfers: import('../types').NftTransfer[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useNftTokenTransfers(contractAddress: string | undefined, tokenId: string | undefined, params: GetNftTransfersParams = {}): UseNftTokenTransfersResult {
  const [transfers, setTransfers] = useState<import('../types').NftTransfer[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTransfers = useCallback(async () => {
    if (!contractAddress || !tokenId) { setLoading(false); return; }
    setLoading(true);
    setError(null);
    try {
      const response = await getNftTokenTransfers(contractAddress, tokenId, params);
      setTransfers(response.data);
      setPagination({ page: response.page, limit: response.limit, total: response.total, total_pages: response.total_pages });
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [contractAddress, tokenId, params.page, params.limit]);

  useEffect(() => { fetchTransfers(); }, [fetchTransfers]);

  return { transfers, pagination, loading, error, refetch: fetchTransfers };
}

interface UseNftCollectionTransfersResult {
  transfers: import('../types').NftTransfer[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useNftCollectionTransfers(contractAddress: string | undefined, params: GetNftTransfersParams = {}): UseNftCollectionTransfersResult {
  const [transfers, setTransfers] = useState<import('../types').NftTransfer[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTransfers = useCallback(async () => {
    if (!contractAddress) { setLoading(false); return; }
    setLoading(true);
    setError(null);
    try {
      const response = await getNftTransfers(contractAddress, params);
      setTransfers(response.data);
      setPagination({ page: response.page, limit: response.limit, total: response.total, total_pages: response.total_pages });
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [contractAddress, params.page, params.limit]);

  useEffect(() => { fetchTransfers(); }, [fetchTransfers]);

  return { transfers, pagination, loading, error, refetch: fetchTransfers };
}

interface UseAddressNftsResult {
  tokens: NftToken[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useAddressNfts(address: string | undefined, params: GetAddressNftsParams = {}): UseAddressNftsResult {
  const [tokens, setTokens] = useState<NftToken[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTokens = useCallback(async () => {
    if (!address) { setLoading(false); return; }
    setLoading(true);
    setError(null);
    try {
      const response = await getAddressNfts(address, params);
      setTokens(response.data);
      setPagination({ page: response.page, limit: response.limit, total: response.total, total_pages: response.total_pages });
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [address, params.page, params.limit]);

  useEffect(() => { fetchTokens(); }, [fetchTokens]);

  return { tokens, pagination, loading, error, refetch: fetchTokens };
}
