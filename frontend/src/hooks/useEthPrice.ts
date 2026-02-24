import { useCallback, useEffect, useState } from 'react';

interface UseEthPriceOptions {
  refreshMs?: number;
}

export default function useEthPrice({ refreshMs = 60000 }: UseEthPriceOptions = {}) {
  const [usd, setUsd] = useState<number | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const loadFromCache = useCallback(() => {
    try {
      const raw = localStorage.getItem('price:eth_usd');
      if (!raw) return null;
      const { v, t } = JSON.parse(raw) as { v: number; t: number };
      if (Date.now() - t < refreshMs) return v;
      return null;
    } catch {
      return null;
    }
  }, [refreshMs]);

  const saveToCache = (v: number) => {
    try {
      localStorage.setItem('price:eth_usd', JSON.stringify({ v, t: Date.now() }));
    } catch {
      return;
    }
  };

  const fetchPrice = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const cached = loadFromCache();
      if (cached != null) {
        setUsd(cached);
        setLoading(false);
        return;
      }

      const key = import.meta.env.VITE_COINGECKO_API_KEY as string | undefined;
      const param = key ? `&x_cg_pro_api_key=${encodeURIComponent(key)}` : '';
      const url = `https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd${param}`;
      let price: number | null = null;
      try {
        const res = await fetch(url);
        if (res.ok) {
          const json = await res.json();
          price = typeof json?.ethereum?.usd === 'number' ? json.ethereum.usd : null;
        }
      } catch {
        price = null;
      }

      // Fallback: Coinbase
      if (price == null) {
        try {
          const res2 = await fetch('https://api.coinbase.com/v2/exchange-rates?currency=ETH');
          if (res2.ok) {
            const json2 = await res2.json();
            const rate = Number(json2?.data?.rates?.USD);
            if (!Number.isNaN(rate) && rate > 0) price = rate;
          }
        } catch {
          price = null;
        }
      }

      if (price == null) throw new Error('Failed to fetch ETH price');
      setUsd(price);
      saveToCache(price);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to fetch price');
    } finally {
      setLoading(false);
    }
  }, [loadFromCache]);

  useEffect(() => {
    fetchPrice();
    const id = setInterval(fetchPrice, refreshMs);
    return () => clearInterval(id);
  }, [fetchPrice, refreshMs]);

  return { usd, loading, error, refetch: fetchPrice };
}
