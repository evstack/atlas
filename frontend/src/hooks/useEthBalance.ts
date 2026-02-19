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
    } catch (e: any) {
      setError(e?.error || e?.message || 'Failed to fetch balance');
    } finally {
      setLoading(false);
    }
  }, [address]);

  useEffect(() => { fetchBalance(); }, [fetchBalance]);

  return { balanceWei, loading, error, refetch: fetchBalance };
}

