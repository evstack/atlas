import darkLogo from './evolve-logo.svg';
import lightLogo from './evolve-logo-light.svg';
import type { Theme } from '../context/theme-context';

export function getDefaultLogo(theme: Theme): string {
  return theme === 'light' ? lightLogo : darkLogo;
}

export { darkLogo as defaultDarkLogo, lightLogo as defaultLightLogo };
