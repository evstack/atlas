import { describe, expect, test } from 'bun:test';
import { resolveLogoUrl } from './branding';

describe('resolveLogoUrl', () => {
  test('uses theme-specific logos when both are present', () => {
    const config = {
      logo_url_light: '/branding/light.svg',
      logo_url_dark: '/branding/dark.svg',
    };

    expect(resolveLogoUrl(config, 'light')).toBe('/branding/light.svg');
    expect(resolveLogoUrl(config, 'dark')).toBe('/branding/dark.svg');
  });

  test('reuses the light logo when dark is missing', () => {
    const config = {
      logo_url_light: '/branding/light.svg',
    };

    expect(resolveLogoUrl(config, 'light')).toBe('/branding/light.svg');
    expect(resolveLogoUrl(config, 'dark')).toBe('/branding/light.svg');
  });

  test('reuses the dark logo when light is missing', () => {
    const config = {
      logo_url_dark: '/branding/dark.svg',
    };

    expect(resolveLogoUrl(config, 'light')).toBe('/branding/dark.svg');
    expect(resolveLogoUrl(config, 'dark')).toBe('/branding/dark.svg');
  });

  test('prefers the shared logo when the current theme-specific logo is missing', () => {
    expect(
      resolveLogoUrl(
        {
          logo_url: '/branding/shared.svg',
          logo_url_dark: '/branding/dark.svg',
        },
        'light',
      ),
    ).toBe('/branding/shared.svg');

    expect(
      resolveLogoUrl(
        {
          logo_url: '/branding/shared.svg',
          logo_url_light: '/branding/light.svg',
        },
        'dark',
      ),
    ).toBe('/branding/shared.svg');
  });

  test('uses the shared logo in both themes when it is the only custom logo', () => {
    const config = {
      logo_url: '/branding/shared.svg',
    };

    expect(resolveLogoUrl(config, 'light')).toBe('/branding/shared.svg');
    expect(resolveLogoUrl(config, 'dark')).toBe('/branding/shared.svg');
  });

  test('returns null when no custom logo is configured', () => {
    expect(resolveLogoUrl({}, 'light')).toBeNull();
    expect(resolveLogoUrl({}, 'dark')).toBeNull();
  });
});
