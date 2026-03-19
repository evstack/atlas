import { useEffect, useState, useContext, type ReactNode } from 'react';
import { getConfig, type BrandingConfig } from '../api/config';
import { deriveSurfaceShades, applyPalette, hexToRgbTriplet } from '../utils/color';
import { ThemeContext } from './theme-context';
import { BrandingContext, brandingDefaults } from './branding-context';

const CACHE_KEY = 'branding_config';

function readCache(): BrandingConfig | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    return raw ? (JSON.parse(raw) as BrandingConfig) : null;
  } catch {
    return null;
  }
}

function applyConfigState(cfg: BrandingConfig, setBranding: (v: typeof brandingDefaults) => void, setConfig: (v: BrandingConfig) => void) {
  setConfig(cfg);
  setBranding({ chainName: cfg.chain_name, logoUrl: cfg.logo_url || null, loaded: true });
  document.title = `${cfg.chain_name} - Block Explorer`;
  if (cfg.logo_url) {
    const link = document.querySelector<HTMLLinkElement>("link[rel='icon']");
    if (link) link.href = cfg.logo_url;
  }
}

export function BrandingProvider({ children }: { children: ReactNode }) {
  const [branding, setBranding] = useState(() => {
    const cached = readCache();
    return cached
      ? { chainName: cached.chain_name, logoUrl: cached.logo_url || null, loaded: true }
      : brandingDefaults;
  });
  const [config, setConfig] = useState<BrandingConfig | null>(readCache);
  const themeCtx = useContext(ThemeContext);
  const theme = themeCtx?.theme ?? 'dark';

  // Apply cached config immediately on mount (no flash), then revalidate in background
  useEffect(() => {
    getConfig()
      .then((cfg) => {
        localStorage.setItem(CACHE_KEY, JSON.stringify(cfg));
        applyConfigState(cfg, setBranding, setConfig);
      })
      .catch(() => {
        if (!config) setBranding({ ...brandingDefaults, loaded: true });
      });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Apply accent + semantic colors (theme-independent)
  useEffect(() => {
    if (!config) return;
    const root = document.documentElement;

    const setRgbVar = (cssVar: string, hex: string | null | undefined) => {
      if (!hex) {
        root.style.removeProperty(cssVar);
        return;
      }
      try {
        root.style.setProperty(cssVar, hexToRgbTriplet(hex));
      } catch {
        root.style.removeProperty(cssVar);
      }
    };

    setRgbVar('--color-accent-primary', config.accent_color);
    setRgbVar('--color-accent-success', config.success_color);
    setRgbVar('--color-accent-error', config.error_color);
  }, [config]);

  // Apply background palette reactively on theme change
  useEffect(() => {
    if (!config) return;

    if (theme === 'dark' && config.background_color_dark) {
      try {
        const palette = deriveSurfaceShades(config.background_color_dark, 'dark');
        applyPalette(palette);
      } catch {
        // fall through to default CSS vars
      }
    } else if (theme === 'light' && config.background_color_light) {
      try {
        const palette = deriveSurfaceShades(config.background_color_light, 'light');
        applyPalette(palette);
      } catch {
        // fall through to default CSS vars
      }
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
      vars.forEach((v) => {
        root.style.removeProperty(v);
      });
    }
  }, [config, theme]);

  return (
    <BrandingContext.Provider value={branding}>
      {children}
    </BrandingContext.Provider>
  );
}
