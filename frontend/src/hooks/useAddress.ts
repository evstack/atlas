import { useState, useEffect, useCallback } from 'react';
import type { Address, ApiError } from '../types';
import { getAddress } from '../api/addresses';

interface UseAddressResult {
  address: Address | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => Promise<void>;
}

export function useAddress(addressHash: string | undefined): UseAddressResult {
  const [address, setAddress] = useState<Address | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchAddress = useCallback(async () => {
    if (!addressHash) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getAddress(addressHash);
      setAddress(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [addressHash]);

  useEffect(() => {
    fetchAddress();
  }, [fetchAddress]);

  return { address, loading, error, refetch: fetchAddress };
}
