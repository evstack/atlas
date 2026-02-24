import { useState, useEffect, useCallback } from 'react';
import { getEthBalance } from '../api/addresses';

interface UseEthBalanceResult {
  balanceWei: string | null;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

export default function useEthBalance(address: string | undefined): UseEthBalanceResult {
  const [balanceWei, setBalanceWei] = useState<string | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const fetchBalance = useCallback(async () => {
    if (!address) { setLoading(false); return; }
    setLoading(true);
    setError(null);
    try {
      const wei = await getEthBalance(address);
      setBalanceWei(wei);
    } catch (e: unknown) {
      const msg = typeof e === 'object' && e !== null && 'error' in e && typeof (e as { error?: unknown }).error === 'string'
        ? (e as { error: string }).error
        : e instanceof Error
          ? e.message
          : 'Failed to fetch balance';
      setError(msg);
    } finally {
      setLoading(false);
    }
  }, [address]);

  useEffect(() => { fetchBalance(); }, [fetchBalance]);

  return { balanceWei, loading, error, refetch: fetchBalance };
}
