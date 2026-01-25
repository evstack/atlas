import { useState, useEffect, useCallback } from 'react';
import type { Transaction, ApiError } from '../types';
import { getTransactions, getTransactionByHash, getTransactionsByAddress } from '../api/transactions';
import type { GetTransactionsParams } from '../api/transactions';

interface UseTransactionsResult {
  transactions: Transaction[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useTransactions(params: GetTransactionsParams = {}): UseTransactionsResult {
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTransactions = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getTransactions(params);
      setTransactions(response.data);
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
  }, [params.page, params.limit, params.block_number, params.address]);

  useEffect(() => {
    fetchTransactions();
  }, [fetchTransactions]);

  return { transactions, pagination, loading, error, refetch: fetchTransactions };
}

export function useAddressTransactions(address: string | undefined, params: { page?: number; limit?: number } = {}): UseTransactionsResult {
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTransactions = useCallback(async () => {
    if (!address) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getTransactionsByAddress(address, params);
      setTransactions(response.data);
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
    fetchTransactions();
  }, [fetchTransactions]);

  return { transactions, pagination, loading, error, refetch: fetchTransactions };
}

interface UseTransactionResult {
  transaction: Transaction | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useTransaction(txHash: string | undefined): UseTransactionResult {
  const [transaction, setTransaction] = useState<Transaction | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTransaction = useCallback(async () => {
    if (!txHash) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getTransactionByHash(txHash);
      setTransaction(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [txHash]);

  useEffect(() => {
    fetchTransaction();
  }, [fetchTransaction]);

  return { transaction, loading, error, refetch: fetchTransaction };
}
