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

interface DerivedBrandTokens {
  brandAqua: string;
  brandLavender: string;
  brandLemon: string;
  brandBlush: string;
  pageGradient: string;
  spectralGradient: string;
}

export function hexToRgbTriplet(hex: string): string {
  const { r, g, b } = hexToRgb(hex);
  return `${r} ${g} ${b}`;
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

function rgbToHex({ r, g, b }: RGB): string {
  return `#${[r, g, b]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("")}`;
}

function mixRgb(a: RGB, b: RGB, weightToB: number): RGB {
  const clamped = Math.max(0, Math.min(1, weightToB));
  const weightToA = 1 - clamped;
  return {
    r: Math.round(a.r * weightToA + b.r * clamped),
    g: Math.round(a.g * weightToA + b.g * clamped),
    b: Math.round(a.b * weightToA + b.b * clamped),
  };
}

function adjustLightness(hsl: HSL, delta: number): RGB {
  return hslToRgb({ ...hsl, l: Math.min(100, Math.max(0, hsl.l + delta)) });
}

function shiftLightness(rgb: RGB, delta: number): RGB {
  return adjustLightness(rgbToHsl(rgb), delta);
}

/**
 * Derive a full surface palette from a single base background color.
 * For dark mode, surfaces are lighter than the base.
 * For light mode, surfaces are darker than the base.
 */
export function deriveSurfaceShades(baseHex: string, mode: 'dark' | 'light', accentHex?: string): DerivedPalette {
  const baseRgb = hexToRgb(baseHex);
  const baseHsl = rgbToHsl(baseRgb);
  const dir = mode === 'dark' ? 1 : -1;

  let surface900 = adjustLightness(baseHsl, dir * 1);
  let surface800 = adjustLightness(baseHsl, dir * 3);
  let surface700 = adjustLightness(baseHsl, dir * 6);
  let surface600 = adjustLightness(baseHsl, dir * 11);
  let surface500 = adjustLightness(baseHsl, dir * 17);
  let border = adjustLightness(baseHsl, dir * 14);

  // Tint surfaces with accent to avoid icy/gray look
  if (accentHex) {
    const accent = hexToRgb(accentHex);
    if (mode === 'light') {
      surface900 = mixRgb(surface900, accent, 0.09);
      surface800 = mixRgb(surface800, accent, 0.13);
      surface700 = mixRgb(surface700, accent, 0.16);
      surface600 = mixRgb(surface600, accent, 0.18);
      surface500 = mixRgb(surface500, accent, 0.20);
      border = mixRgb(border, accent, 0.22);
    } else {
      surface900 = mixRgb(surface900, accent, 0.06);
      surface800 = mixRgb(surface800, accent, 0.09);
      surface700 = mixRgb(surface700, accent, 0.11);
      surface600 = mixRgb(surface600, accent, 0.13);
      surface500 = mixRgb(surface500, accent, 0.15);
      border = mixRgb(border, accent, 0.16);
    }
  }

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

export function deriveBrandTokens(
  backgroundHex: string,
  accentHex: string,
  mode: 'dark' | 'light',
): DerivedBrandTokens {
  const background = hexToRgb(backgroundHex);
  const accent = hexToRgb(accentHex);
  const white = hexToRgb('#ffffff');

  const brandAqua = mode === 'dark'
    ? shiftLightness(mixRgb(background, accent, 0.48), 12)
    : shiftLightness(mixRgb(background, accent, 0.52), -2);
  const brandLavender = mode === 'dark'
    ? shiftLightness(mixRgb(background, accent, 0.26), 8)
    : shiftLightness(mixRgb(background, accent, 0.22), -4);
  const brandLemon = mode === 'dark'
    ? shiftLightness(mixRgb(accent, white, 0.12), 6)
    : shiftLightness(mixRgb(accent, white, 0.28), 2);
  const brandBlush = mode === 'dark'
    ? shiftLightness(mixRgb(accent, background, 0.46), 10)
    : shiftLightness(mixRgb(background, accent, 0.18), 6);

  const pageStart = mode === 'dark'
    ? rgbToHex(shiftLightness(background, -2))
    : backgroundHex;
  const pageEnd = mode === 'dark'
    ? rgbToHex(shiftLightness(background, 4))
    : rgbToHex(shiftLightness(background, -4));
  const pageGradient = mode === 'dark'
    ? `radial-gradient(circle at 18% 85%, rgb(${rgbTriplet(brandLavender)} / 0.26), transparent 30%), radial-gradient(circle at 82% 20%, rgb(${rgbTriplet(brandLemon)} / 0.20), transparent 26%), radial-gradient(circle at 30% 14%, rgb(${rgbTriplet(brandAqua)} / 0.28), transparent 28%), linear-gradient(180deg, ${pageStart} 0%, ${pageEnd} 100%)`
    : `radial-gradient(circle at 8% 78%, rgb(${rgbTriplet(brandLavender)} / 0.28), transparent 28%), radial-gradient(circle at 80% 18%, rgb(${rgbTriplet(brandLemon)} / 0.24), transparent 24%), radial-gradient(circle at 32% 18%, rgb(${rgbTriplet(brandAqua)} / 0.5), transparent 30%), linear-gradient(180deg, ${pageStart} 0%, ${pageEnd} 100%)`;
  const spectralGradient = `linear-gradient(120deg, rgb(${rgbTriplet(brandAqua)} / 0.95) 0%, rgb(${rgbTriplet(brandAqua)} / 0.72) 25%, rgb(${rgbTriplet(brandLemon)} / 0.88) 55%, rgb(${rgbTriplet(brandBlush)} / 0.72) 82%, rgb(${rgbTriplet(brandLavender)} / 0.84) 100%)`;

  return {
    brandAqua: rgbTriplet(brandAqua),
    brandLavender: rgbTriplet(brandLavender),
    brandLemon: rgbTriplet(brandLemon),
    brandBlush: rgbTriplet(brandBlush),
    pageGradient,
    spectralGradient,
  };
}

export function applyBrandTokens(tokens: DerivedBrandTokens) {
  const root = document.documentElement;

  root.style.setProperty('--color-brand-aqua', tokens.brandAqua);
  root.style.setProperty('--color-brand-lavender', tokens.brandLavender);
  root.style.setProperty('--color-brand-lemon', tokens.brandLemon);
  root.style.setProperty('--color-brand-blush', tokens.brandBlush);
  root.style.setProperty('--page-gradient', tokens.pageGradient);
  root.style.setProperty('--spectral-gradient', tokens.spectralGradient);
}

export function clearBrandTokens() {
  const root = document.documentElement;
  const vars = [
    '--color-brand-aqua',
    '--color-brand-lavender',
    '--color-brand-lemon',
    '--color-brand-blush',
    '--page-gradient',
    '--spectral-gradient',
  ];

  vars.forEach((cssVar) => root.style.removeProperty(cssVar));
}
