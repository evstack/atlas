import { useState, useEffect, useCallback } from 'react';
import type { AddressTransfer, ApiError } from '../types';
import { getAddressTransfers, type GetAddressTransfersParams } from '../api/addresses';

interface UseAddressTransfersResult {
  transfers: AddressTransfer[];
  pagination: { page: number; limit: number; total: number; total_pages: number } | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useAddressTransfers(address: string | undefined, params: GetAddressTransfersParams = {}): UseAddressTransfersResult {
  const [transfers, setTransfers] = useState<AddressTransfer[]>([]);
  const [pagination, setPagination] = useState<{ page: number; limit: number; total: number; total_pages: number } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchTransfers = useCallback(async () => {
    if (!address) { setLoading(false); return; }
    setLoading(true);
    setError(null);
    try {
      const response = await getAddressTransfers(address, params);
      setTransfers(response.data);
      setPagination({ page: response.page, limit: response.limit, total: response.total, total_pages: response.total_pages });
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [address, params.page, params.limit, params.transfer_type]);

  useEffect(() => { fetchTransfers(); }, [fetchTransfers]);

  return { transfers, pagination, loading, error, refetch: fetchTransfers };
}

