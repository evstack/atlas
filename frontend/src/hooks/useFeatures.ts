import { useEffect, useState } from 'react';
import { getStatus } from '../api/status';
import type { ChainFeatures } from '../types';

const defaultFeatures: ChainFeatures = { da_tracking: false };

/**
 * Fetches chain feature flags from /api/status once on mount.
 * Returns the features object (defaults to all disabled until loaded).
 */
export default function useFeatures(): ChainFeatures {
  const [features, setFeatures] = useState<ChainFeatures>(defaultFeatures);

  useEffect(() => {
    let cancelled = false;
    getStatus().then((status) => {
      if (!cancelled && status.features) {
        setFeatures(status.features);
      }
    }).catch(() => {
      // Silently use defaults on error
    });
    return () => { cancelled = true; };
  }, []);

  return features;
}
