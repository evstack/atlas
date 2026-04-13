import { useState, useEffect, useCallback, useRef } from 'react';
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
  const requestIdRef = useRef(0);

  const fetchContract = useCallback(async () => {
    const requestId = ++requestIdRef.current;

    if (!address) {
      setContract(null);
      setError(null);
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const response = await getContractDetail(address);
      if (requestId !== requestIdRef.current) return;
      setContract(response);
    } catch (err) {
      if (requestId !== requestIdRef.current) return;
      setContract(null);
      setError(err as ApiError);
    } finally {
      if (requestId === requestIdRef.current) {
        setLoading(false);
      }
    }
  }, [address]);

  useEffect(() => {
    fetchContract();
  }, [fetchContract]);

  return { contract, loading, error, refetch: fetchContract };
}
