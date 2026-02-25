import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import { getConfig, type BrandingConfig } from '../api/config';
import { deriveSurfaceShades, applyPalette } from '../utils/color';
import { ThemeContext } from './theme-context';

interface BrandingContextValue {
  chainName: string;
  logoUrl: string | null;
  loaded: boolean;
}

const defaults: BrandingContextValue = {
  chainName: 'Atlas',
  logoUrl: null,
  loaded: false,
};

const BrandingContext = createContext<BrandingContextValue>(defaults);

export function BrandingProvider({ children }: { children: ReactNode }) {
  const [branding, setBranding] = useState<BrandingContextValue>(defaults);
  const [config, setConfig] = useState<BrandingConfig | null>(null);
  const themeCtx = useContext(ThemeContext);
  const theme = themeCtx?.theme ?? 'dark';

  // Fetch config once on mount
  useEffect(() => {
    getConfig()
      .then((cfg) => {
        setConfig(cfg);
        setBranding({
          chainName: cfg.chain_name,
          logoUrl: cfg.logo_url || null,
          loaded: true,
        });

        // Update page title
        document.title = `${cfg.chain_name} - Block Explorer`;

        // Update favicon if logo_url is set
        if (cfg.logo_url) {
          const link = document.querySelector<HTMLLinkElement>("link[rel='icon']");
          if (link) {
            link.href = cfg.logo_url;
          }
        }
      })
      .catch(() => {
        setBranding({ ...defaults, loaded: true });
      });
  }, []);

  // Apply accent + semantic colors (theme-independent)
  useEffect(() => {
    if (!config) return;
    const root = document.documentElement;

    if (config.accent_color) {
      root.style.setProperty('--color-accent-primary', config.accent_color);
    }
    if (config.success_color) {
      root.style.setProperty('--color-accent-success', config.success_color);
    }
    if (config.error_color) {
      root.style.setProperty('--color-accent-error', config.error_color);
    }
  }, [config]);

  // Apply background palette reactively on theme change
  useEffect(() => {
    if (!config) return;

    if (theme === 'dark' && config.background_color_dark) {
      const palette = deriveSurfaceShades(config.background_color_dark, 'dark');
      applyPalette(palette, 'dark');
    } else if (theme === 'light' && config.background_color_light) {
      const palette = deriveSurfaceShades(config.background_color_light, 'light');
      applyPalette(palette, 'light');
    } else {
      // Remove any inline overrides so the CSS defaults take effect
      const root = document.documentElement;
      const vars = [
        '--color-surface-900', '--color-surface-800', '--color-surface-700',
        '--color-surface-600', '--color-surface-500', '--color-body-bg',
        '--color-body-text', '--color-border', '--color-text-primary',
        '--color-text-secondary', '--color-text-muted', '--color-text-subtle',
        '--color-text-faint',
      ];
      vars.forEach(v => root.style.removeProperty(v));
    }
  }, [config, theme]);

  return (
    <BrandingContext.Provider value={branding}>
      {children}
    </BrandingContext.Provider>
  );
}

export function useBranding() {
  return useContext(BrandingContext);
}
