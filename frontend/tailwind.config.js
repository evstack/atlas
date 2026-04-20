/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        mono: ['JetBrains Mono', 'Geist Mono', 'SFMono-Regular', 'Menlo', 'monospace'],
        sans: ['Inter', 'Helvetica Neue', 'Helvetica', 'Arial', 'sans-serif'],
        display: ['Inter', 'Helvetica Neue', 'Helvetica', 'Arial', 'sans-serif'],
        brandmono: ['Geist Mono', 'JetBrains Mono', 'monospace'],
      },
      colors: {
        brand: {
          aqua: 'rgb(var(--color-brand-aqua) / <alpha-value>)',
          lavender: 'rgb(var(--color-brand-lavender) / <alpha-value>)',
          lemon: 'rgb(var(--color-brand-lemon) / <alpha-value>)',
          blush: 'rgb(var(--color-brand-blush) / <alpha-value>)',
        },
        dark: {
          900: 'rgb(var(--color-surface-900) / <alpha-value>)',
          800: 'rgb(var(--color-surface-800) / <alpha-value>)',
          700: 'rgb(var(--color-surface-700) / <alpha-value>)',
          600: 'rgb(var(--color-surface-600) / <alpha-value>)',
          500: 'rgb(var(--color-surface-500) / <alpha-value>)',
        },
        surface: {
          900: 'rgb(var(--color-surface-900) / <alpha-value>)',
          800: 'rgb(var(--color-surface-800) / <alpha-value>)',
          700: 'rgb(var(--color-surface-700) / <alpha-value>)',
          600: 'rgb(var(--color-surface-600) / <alpha-value>)',
          500: 'rgb(var(--color-surface-500) / <alpha-value>)',
        },
        border: {
          DEFAULT: 'rgb(var(--color-border) / <alpha-value>)',
        },
        fg: {
          DEFAULT: 'rgb(var(--color-text-primary) / <alpha-value>)',
          secondary: 'rgb(var(--color-text-secondary) / <alpha-value>)',
          muted: 'rgb(var(--color-text-muted) / <alpha-value>)',
          subtle: 'rgb(var(--color-text-subtle) / <alpha-value>)',
          faint: 'rgb(var(--color-text-faint) / <alpha-value>)',
        },
        gray: {
          200: 'rgb(var(--color-gray-200) / <alpha-value>)',
          300: 'rgb(var(--color-gray-300) / <alpha-value>)',
          400: 'rgb(var(--color-gray-400) / <alpha-value>)',
          500: 'rgb(var(--color-gray-500) / <alpha-value>)',
          600: 'rgb(var(--color-gray-600) / <alpha-value>)',
          700: 'rgb(var(--color-gray-700) / <alpha-value>)',
        },
        accent: {
          primary: 'rgb(var(--color-accent-primary, 220 38 38) / <alpha-value>)',
          secondary: 'rgb(var(--color-accent-primary, 220 38 38) / <alpha-value>)',
          success: 'rgb(var(--color-accent-success, 34 197 94) / <alpha-value>)',
          warning: 'rgb(var(--color-accent-primary, 220 38 38) / <alpha-value>)',
          error: 'rgb(var(--color-accent-error, 220 38 38) / <alpha-value>)',
        },
      },
    },
  },
  plugins: [],
}
