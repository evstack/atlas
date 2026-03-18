import { useEffect, useState } from 'react';
import { getStatus } from '../api/status';
import type { ChainFeatures } from '../types';

const defaultFeatures: ChainFeatures = { da_tracking: false };
type FeaturesListener = (features: ChainFeatures) => void;

let cachedFeatures: ChainFeatures | null = null;
let featuresPromise: Promise<ChainFeatures> | null = null;
const listeners = new Set<FeaturesListener>();

function notifyFeatures(features: ChainFeatures) {
  for (const listener of listeners) {
    listener(features);
  }
}

function loadFeatures(): Promise<ChainFeatures> {
  if (cachedFeatures) {
    return Promise.resolve(cachedFeatures);
  }

  if (!featuresPromise) {
    featuresPromise = getStatus()
      .then((status) => status.features ?? defaultFeatures)
      .catch(() => defaultFeatures)
      .then((features) => {
        cachedFeatures = features;
        notifyFeatures(features);
        return features;
      })
      .finally(() => {
        featuresPromise = null;
      });
  }

  return featuresPromise;
}

/**
 * Returns cached chain feature flags, loading them only once per app session.
 */
export default function useFeatures(): ChainFeatures {
  const [features, setFeatures] = useState<ChainFeatures>(cachedFeatures ?? defaultFeatures);

  useEffect(() => {
    let active = true;
    const listener: FeaturesListener = (nextFeatures) => {
      if (active) {
        setFeatures(nextFeatures);
      }
    };

    listeners.add(listener);
    void loadFeatures().then((nextFeatures) => {
      if (active) {
        setFeatures(nextFeatures);
      }
    });

    return () => {
      active = false;
      listeners.delete(listener);
    };
  }, []);

  return features;
}
