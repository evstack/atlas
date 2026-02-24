import { useState, useEffect, useCallback, useRef } from 'react';
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
  const paramsRef = useRef(params);
  const paramsKey = JSON.stringify(params);

  useEffect(() => {
    paramsRef.current = params;
  }, [params]);

  const fetchBlocks = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getBlocks(paramsRef.current);
      setBlocks(response.data);
      setPagination(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchBlocks();
  }, [fetchBlocks, paramsKey]);

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

  const paramsRef = useRef(params);
  const txParamsKey = JSON.stringify(params);

  useEffect(() => {
    paramsRef.current = params;
  }, [params]);

  const fetchTransactions = useCallback(async () => {
    if (blockNumber === undefined) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getBlockTransactions(blockNumber, paramsRef.current);
      setTransactions(response.data);
      setPagination(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [blockNumber]);

  useEffect(() => {
    fetchTransactions();
  }, [fetchTransactions, txParamsKey]);

  return { transactions, pagination, loading, error, refetch: fetchTransactions };
}
