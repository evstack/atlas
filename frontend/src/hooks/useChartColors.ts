import { useContext, useMemo } from 'react';
import { BrandingContext } from '../context/branding-context';
import { ThemeContext } from '../context/theme-context';

function cssVar(name: string): string {
  return `rgb(${getComputedStyle(document.documentElement).getPropertyValue(name).trim()})`;
}

export function useChartColors() {
  const { accentHex } = useContext(BrandingContext);
  const themeCtx = useContext(ThemeContext);
  const theme = themeCtx?.theme ?? 'dark';

  return useMemo(() => ({
    accent: accentHex ?? cssVar('--color-accent-primary'),
    grid: cssVar('--color-surface-600'),
    axisText: cssVar('--color-text-subtle'),
    tooltipBg: cssVar('--color-surface-800'),
    tooltipText: cssVar('--color-text-primary'),
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [accentHex, theme]);
}
