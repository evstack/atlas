import { describe, expect, test } from 'bun:test';
import { deriveBrandTokens, deriveSurfaceShades, hexToRgbTriplet } from './color';

describe('hexToRgbTriplet', () => {
  test('converts hex into a CSS rgb triplet', () => {
    expect(hexToRgbTriplet('#94EEFF')).toBe('148 238 255');
    expect(hexToRgbTriplet('#000000')).toBe('0 0 0');
  });
});

describe('deriveSurfaceShades', () => {
  test('derives a readable light palette from a base hex color', () => {
    const palette = deriveSurfaceShades('#F3F4F4', 'light');

    expect(palette.bodyBg).toBe('#F3F4F4');
    expect(palette.bodyText).toBe('#1f1f1f');
    expect(palette.textPrimary).toBe('31 31 31');
    expect(palette.surface900).not.toBe('');
  });

  test('derives a readable dark palette from a base hex color', () => {
    const palette = deriveSurfaceShades('#090B0C', 'dark');

    expect(palette.bodyBg).toBe('#090B0C');
    expect(palette.bodyText).toBe('#f8fafc');
    expect(palette.textPrimary).toBe('248 250 252');
    expect(palette.surface900).not.toBe('');
  });
});

describe('deriveBrandTokens', () => {
  test('derives motif colors and gradients from light branding inputs', () => {
    const tokens = deriveBrandTokens('#ECF3EF', '#F1BE5A', 'light');

    expect(tokens.brandLemon).not.toBe('');
    expect(tokens.brandLemon).not.toBe(tokens.brandLavender);
    expect(tokens.brandAqua).not.toBe('');
    expect(tokens.pageGradient).toContain('linear-gradient(180deg');
    expect(tokens.spectralGradient).toContain('linear-gradient');
  });

  test('derives motif colors and gradients from dark branding inputs', () => {
    const tokens = deriveBrandTokens('#245636', '#F1BE5A', 'dark');

    expect(tokens.brandLavender).not.toBe(tokens.brandLemon);
    expect(tokens.pageGradient).toContain('linear-gradient(180deg');
    expect(tokens.spectralGradient).toContain('rgb(');
  });
});
