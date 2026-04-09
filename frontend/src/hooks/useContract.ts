import { useState, useEffect, useCallback } from 'react';
import type { ContractDetail, ApiError } from '../types';
import { getContractDetail } from '../api/contracts';

interface UseContractResult {
  contract: ContractDetail | null;
  loading: boolean;
  error: ApiError | null;
  refetch: () => void;
}

export function useContract(address: string | undefined): UseContractResult {
  const [contract, setContract] = useState<ContractDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);

  const fetchContract = useCallback(async () => {
    if (!address) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getContractDetail(address);
      setContract(response);
    } catch (err) {
      setError(err as ApiError);
    } finally {
      setLoading(false);
    }
  }, [address]);

  useEffect(() => {
    fetchContract();
  }, [fetchContract]);

  return { contract, loading, error, refetch: fetchContract };
}
