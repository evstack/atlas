import type { BrandingConfig } from '../api/config';
import type { BrandingContextValue } from './branding-context';
import type { Theme } from './theme-context';

export function resolveLogoUrl(
  config: Pick<BrandingConfig, 'logo_url' | 'logo_url_light' | 'logo_url_dark'>,
  theme: Theme,
): string | null {
  if (theme === 'light') {
    return config.logo_url_light ?? config.logo_url ?? config.logo_url_dark ?? null;
  }

  return config.logo_url_dark ?? config.logo_url ?? config.logo_url_light ?? null;
}

export function resolveBrandingValue(
  config: BrandingConfig,
  theme: Theme,
): BrandingContextValue {
  return {
    chainName: config.chain_name,
    logoUrl: resolveLogoUrl(config, theme),
    loaded: true,
  };
}
