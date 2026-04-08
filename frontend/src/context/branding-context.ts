import { createContext } from "react";
import type { FaucetConfig } from "../api/config";
import type { ChainFeatures } from "../types";

export interface BrandingContextValue {
  chainName: string;
  logoUrl: string | null;
  accentHex: string | null;
  features: ChainFeatures;
  faucet: FaucetConfig;
  loaded: boolean;
}

export const brandingDefaults: BrandingContextValue = {
  chainName: "Unknown",
  logoUrl: null,
  accentHex: null,
  features: { da_tracking: false },
  faucet: { enabled: false },
  loaded: false,
};

export const BrandingContext =
  createContext<BrandingContextValue>(brandingDefaults);
