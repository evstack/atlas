import { createContext } from 'react';

export interface BrandingContextValue {
  chainName: string;
  logoUrl: string | null;
  loaded: boolean;
}

export const brandingDefaults: BrandingContextValue = {
  chainName: 'Atlas',
  logoUrl: null,
  loaded: false,
};

export const BrandingContext = createContext<BrandingContextValue>(brandingDefaults);
