import { createContext } from 'react';

export type Theme = 'dark' | 'light';

export type ThemeContextValue = {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
};

export const STORAGE_KEY = 'atlas-theme';

export const ThemeContext = createContext<ThemeContextValue | undefined>(undefined);
