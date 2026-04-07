import { createContext } from 'react';

export interface BrandingContextValue {
  chainName: string;
  logoUrl: string | null;
  accentHex: string | null;
  loaded: boolean;
}

export const brandingDefaults: BrandingContextValue = {
  chainName: 'Unknown',
  logoUrl: null,
  accentHex: null,
  loaded: false,
};

export const BrandingContext = createContext<BrandingContextValue>(brandingDefaults);
