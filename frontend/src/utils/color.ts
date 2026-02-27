/**
 * Color utilities for white-label theming.
 * Derives surface shade palettes from a single base background color.
 */

interface RGB {
  r: number;
  g: number;
  b: number;
}

interface HSL {
  h: number;
  s: number;
  l: number;
}

interface DerivedPalette {
  surface900: string; // RGB triplet "r g b"
  surface800: string;
  surface700: string;
  surface600: string;
  surface500: string;
  bodyBg: string; // hex
  bodyText: string; // hex
  border: string; // RGB triplet
  textPrimary: string; // RGB triplet
  textSecondary: string;
  textMuted: string;
  textSubtle: string;
  textFaint: string;
}

function hexToRgb(hex: string): RGB {
  const raw = hex.trim().replace(/^#/, '');
  const clean = raw.length === 3
    ? raw.split('').map((c) => c + c).join('')
    : raw;

  if (!/^[0-9a-fA-F]{6}$/.test(clean)) {
    throw new Error(`Invalid hex color: "${hex}"`);
  }

  return {
    r: parseInt(clean.slice(0, 2), 16),
    g: parseInt(clean.slice(2, 4), 16),
    b: parseInt(clean.slice(4, 6), 16),
  };
}

function rgbToHsl({ r, g, b }: RGB): HSL {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  let h = 0;
  let s = 0;

  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    switch (max) {
      case rn:
        h = ((gn - bn) / d + (gn < bn ? 6 : 0)) / 6;
        break;
      case gn:
        h = ((bn - rn) / d + 2) / 6;
        break;
      case bn:
        h = ((rn - gn) / d + 4) / 6;
        break;
    }
  }

  return { h: h * 360, s: s * 100, l: l * 100 };
}

function hslToRgb({ h, s, l }: HSL): RGB {
  const sn = s / 100;
  const ln = l / 100;
  const c = (1 - Math.abs(2 * ln - 1)) * sn;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = ln - c / 2;
  let rn = 0, gn = 0, bn = 0;

  if (h < 60) { rn = c; gn = x; }
  else if (h < 120) { rn = x; gn = c; }
  else if (h < 180) { gn = c; bn = x; }
  else if (h < 240) { gn = x; bn = c; }
  else if (h < 300) { rn = x; bn = c; }
  else { rn = c; bn = x; }

  return {
    r: Math.round((rn + m) * 255),
    g: Math.round((gn + m) * 255),
    b: Math.round((bn + m) * 255),
  };
}

function rgbTriplet(rgb: RGB): string {
  return `${rgb.r} ${rgb.g} ${rgb.b}`;
}

function adjustLightness(hsl: HSL, delta: number): RGB {
  return hslToRgb({ ...hsl, l: Math.min(100, Math.max(0, hsl.l + delta)) });
}

/**
 * Derive a full surface palette from a single base background color.
 * For dark mode, surfaces are lighter than the base.
 * For light mode, surfaces are darker than the base.
 */
export function deriveSurfaceShades(baseHex: string, mode: 'dark' | 'light'): DerivedPalette {
  const baseRgb = hexToRgb(baseHex);
  const baseHsl = rgbToHsl(baseRgb);
  const dir = mode === 'dark' ? 1 : -1;

  const surface900 = adjustLightness(baseHsl, dir * 1);
  const surface800 = adjustLightness(baseHsl, dir * 3);
  const surface700 = adjustLightness(baseHsl, dir * 6);
  const surface600 = adjustLightness(baseHsl, dir * 11);
  const surface500 = adjustLightness(baseHsl, dir * 17);
  const border = adjustLightness(baseHsl, dir * 14);

  // Text colors: neutral grays with good contrast
  if (mode === 'dark') {
    return {
      surface900: rgbTriplet(surface900),
      surface800: rgbTriplet(surface800),
      surface700: rgbTriplet(surface700),
      surface600: rgbTriplet(surface600),
      surface500: rgbTriplet(surface500),
      bodyBg: baseHex,
      bodyText: '#f8fafc',
      border: rgbTriplet(border),
      textPrimary: '248 250 252',
      textSecondary: '229 231 235',
      textMuted: '203 213 225',
      textSubtle: '148 163 184',
      textFaint: '100 116 139',
    };
  } else {
    return {
      surface900: rgbTriplet(surface900),
      surface800: rgbTriplet(surface800),
      surface700: rgbTriplet(surface700),
      surface600: rgbTriplet(surface600),
      surface500: rgbTriplet(surface500),
      bodyBg: baseHex,
      bodyText: '#1f1f1f',
      border: rgbTriplet(border),
      textPrimary: '31 31 31',
      textSecondary: '54 54 54',
      textMuted: '88 88 88',
      textSubtle: '120 120 120',
      textFaint: '150 150 150',
    };
  }
}

/**
 * Apply a derived palette to the document root as CSS custom properties.
 */
export function applyPalette(palette: DerivedPalette) {
  const root = document.documentElement;

  // For dark mode, set on :root directly.
  // For light mode, we set the same vars — they'll be active when data-theme='light'
  // is set because the BrandingContext applies them reactively on theme change.
  const setVar = (name: string, value: string) => root.style.setProperty(name, value);

  setVar('--color-surface-900', palette.surface900);
  setVar('--color-surface-800', palette.surface800);
  setVar('--color-surface-700', palette.surface700);
  setVar('--color-surface-600', palette.surface600);
  setVar('--color-surface-500', palette.surface500);
  setVar('--color-body-bg', palette.bodyBg);
  setVar('--color-body-text', palette.bodyText);
  setVar('--color-border', palette.border);
  setVar('--color-text-primary', palette.textPrimary);
  setVar('--color-text-secondary', palette.textSecondary);
  setVar('--color-text-muted', palette.textMuted);
  setVar('--color-text-subtle', palette.textSubtle);
  setVar('--color-text-faint', palette.textFaint);
}
