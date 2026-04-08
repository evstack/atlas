import { useCallback, useEffect, useState } from "react";
import { getFaucetInfo } from "../api/faucet";
import type { ApiError, FaucetInfo } from "../types";
import { toApiError } from "../utils";

export interface UseFaucetInfoResult {
  faucetInfo: FaucetInfo | null;
  loading: boolean;
  error: ApiError | null;
  notFound: boolean;
  refetch: (options?: { background?: boolean }) => Promise<void>;
}

export default function useFaucetInfo(enabled = true): UseFaucetInfoResult {
  const [faucetInfo, setFaucetInfo] = useState<FaucetInfo | null>(null);
  const [loading, setLoading] = useState(enabled);
  const [error, setError] = useState<ApiError | null>(null);
  const [notFound, setNotFound] = useState(false);

  const fetchInfo = useCallback(
    async (options?: { background?: boolean }) => {
      if (!enabled) {
        setFaucetInfo(null);
        setLoading(false);
        setError(null);
        setNotFound(false);
        return;
      }

      const background = options?.background ?? false;
      if (!background) {
        setLoading(true);
        setError(null);
        setNotFound(false);
      }

      try {
        const info = await getFaucetInfo();
        setFaucetInfo(info);
      } catch (err: unknown) {
        if (background) return;
        const apiError = toApiError(err, "Failed to load faucet information");
        if (apiError.status === 404) {
          setFaucetInfo(null);
          setNotFound(true);
        } else {
          setError(apiError);
        }
      } finally {
        if (!background) {
          setLoading(false);
        }
      }
    },
    [enabled],
  );

  useEffect(() => {
    if (!enabled) {
      setFaucetInfo(null);
      setLoading(false);
      setError(null);
      setNotFound(false);
      return;
    }

    void fetchInfo();
  }, [enabled, fetchInfo]);

  return { faucetInfo, loading, error, notFound, refetch: fetchInfo };
}
