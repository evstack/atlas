import { useState, useEffect, useCallback } from 'react';
import type { Address, ApiError, PaginatedResponse } from '../types';
import { getAddresses, type GetAddressesParams } from '../api/addresses';

interface UseAddressesResult {
  addresses: Address[];
  pagination: PaginatedResponse<Address> | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useAddresses(params: GetAddressesParams = {}): UseAddressesResult {
  const [addresses, setAddresses] = useState<Address[]>([]);
  const [pagination, setPagination] = useState<PaginatedResponse<Address> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchAddresses = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getAddresses(params);
      setAddresses(response.data);
      setPagination(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [params.page, params.limit, params.address_type, params.from_block, params.to_block]);

  useEffect(() => {
    fetchAddresses();
  }, [fetchAddresses]);

  return { addresses, pagination, loading, error, refetch: fetchAddresses };
}
