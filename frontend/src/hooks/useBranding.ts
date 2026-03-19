import { useContext } from 'react';
import { BrandingContext } from '../context/branding-context';

export function useBranding() {
  return useContext(BrandingContext);
}
