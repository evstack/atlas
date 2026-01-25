import { useState, useEffect, useCallback } from 'react';
import type { Block, Transaction, PaginatedResponse, ApiError } from '../types';
import { getBlocks, getBlockByNumber, getBlockTransactions } from '../api/blocks';
import type { GetBlocksParams } from '../api/blocks';

interface UseBlocksResult {
  blocks: Block[];
  pagination: PaginatedResponse<Block> | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useBlocks(params: GetBlocksParams = {}): UseBlocksResult {
  const [blocks, setBlocks] = useState<Block[]>([]);
  const [pagination, setPagination] = useState<PaginatedResponse<Block> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchBlocks = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getBlocks(params);
      setBlocks(response.data);
      setPagination(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [params.page, params.limit]);

  useEffect(() => {
    fetchBlocks();
  }, [fetchBlocks]);

  return { blocks, pagination, loading, error, refetch: fetchBlocks };
}

interface UseBlockResult {
  block: Block | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useBlock(blockNumber: number | undefined): UseBlockResult {
  const [block, setBlock] = useState<Block | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchBlock = useCallback(async () => {
    if (blockNumber === undefined) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getBlockByNumber(blockNumber);
      setBlock(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [blockNumber]);

  useEffect(() => {
    fetchBlock();
  }, [fetchBlock]);

  return { block, loading, error, refetch: fetchBlock };
}

interface UseBlockTransactionsResult {
  transactions: Transaction[];
  pagination: PaginatedResponse<Transaction> | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useBlockTransactions(
  blockNumber: number | undefined,
  params: GetBlocksParams = {}
): UseBlockTransactionsResult {
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [pagination, setPagination] = useState<PaginatedResponse<Transaction> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTransactions = useCallback(async () => {
    if (blockNumber === undefined) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getBlockTransactions(blockNumber, params);
      setTransactions(response.data);
      setPagination(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [blockNumber, params.page, params.limit]);

  useEffect(() => {
    fetchTransactions();
  }, [fetchTransactions]);

  return { transactions, pagination, loading, error, refetch: fetchTransactions };
}
