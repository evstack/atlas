import { useState, useEffect, useCallback, useRef } from 'react';
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
  const paramsRef = useRef(params);
  const paramsKey = JSON.stringify(params);

  useEffect(() => {
    paramsRef.current = params;
  }, [params]);

  const fetchAddresses = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getAddresses(paramsRef.current);
      setAddresses(response.data);
      setPagination(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAddresses();
  }, [fetchAddresses, paramsKey]);

  return { addresses, pagination, loading, error, refetch: fetchAddresses };
}
