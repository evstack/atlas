import { useState, useEffect, useCallback } from 'react';
import type { EventLog, DecodedEventLog, ApiError } from '../types';
import {
  getTransactionLogs,
  getTransactionDecodedLogs,
  getAddressLogs,
} from '../api/logs';
import type { GetTransactionLogsParams, GetAddressLogsParams } from '../api/logs';

interface UseTransactionLogsResult {
  logs: EventLog[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useTransactionLogs(txHash: string | undefined, params: GetTransactionLogsParams = {}): UseTransactionLogsResult {
  const [logs, setLogs] = useState<EventLog[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchLogs = useCallback(async () => {
    if (!txHash) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getTransactionLogs(txHash, params);
      setLogs(response.data);
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
  }, [txHash, params.page, params.limit]);

  useEffect(() => {
    fetchLogs();
  }, [fetchLogs]);

  return { logs, pagination, loading, error, refetch: fetchLogs };
}

interface UseTransactionDecodedLogsResult {
  logs: DecodedEventLog[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useTransactionDecodedLogs(txHash: string | undefined, params: GetTransactionLogsParams = {}): UseTransactionDecodedLogsResult {
  const [logs, setLogs] = useState<DecodedEventLog[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchLogs = useCallback(async () => {
    if (!txHash) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getTransactionDecodedLogs(txHash, params);
      setLogs(response.data);
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
  }, [txHash, params.page, params.limit]);

  useEffect(() => {
    fetchLogs();
  }, [fetchLogs]);

  return { logs, pagination, loading, error, refetch: fetchLogs };
}

interface UseAddressLogsResult {
  logs: EventLog[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useAddressLogs(address: string | undefined, params: GetAddressLogsParams = {}): UseAddressLogsResult {
  const [logs, setLogs] = useState<EventLog[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchLogs = useCallback(async () => {
    if (!address) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getAddressLogs(address, params);
      setLogs(response.data);
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
    fetchLogs();
  }, [fetchLogs]);

  return { logs, pagination, loading, error, refetch: fetchLogs };
}
