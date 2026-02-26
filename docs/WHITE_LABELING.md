# White Labeling

Atlas supports white-labeling so each L2 chain can customize the explorer's appearance — name, logo, and color scheme — without rebuilding the frontend.

All branding is configured through environment variables. When none are set, Atlas uses its default branding.

## Configuration

Add these variables to your `.env` file alongside `RPC_URL`:

| Variable | Description | Default |
|----------|-------------|---------|
| `CHAIN_NAME` | Displayed in the navbar, page title, and welcome page | `Atlas` |
| `CHAIN_LOGO_URL` | URL or path to your logo (e.g. `/branding/logo.svg`) | Bundled Atlas logo |
| `ACCENT_COLOR` | Primary accent hex for links, buttons, active states | `#dc2626` |
| `BACKGROUND_COLOR_DARK` | Dark mode base background hex | `#050505` |
| `BACKGROUND_COLOR_LIGHT` | Light mode base background hex | `#f4ede6` |
| `SUCCESS_COLOR` | Success indicator hex (e.g. confirmed badges) | `#22c55e` |
| `ERROR_COLOR` | Error indicator hex (e.g. failed badges) | `#dc2626` |

All variables are optional. Unset variables fall back to the Atlas defaults shown above.

## Custom Logo

To use a custom logo, place your image file in a `branding/` directory at the project root and set `CHAIN_LOGO_URL` to its path:

```
atlas/
├── branding/
│   └── logo.svg      # Your custom logo
├── .env
├── docker-compose.yml
└── ...
```

```env
CHAIN_LOGO_URL=/branding/logo.svg
```

The logo appears in the navbar, the welcome page, and as the browser favicon.

### Docker

In Docker, the `branding/` directory is mounted into the frontend container as a read-only volume. This is configured automatically in `docker-compose.yml`:

```yaml
atlas-frontend:
  volumes:
    - ${BRANDING_DIR:-./branding}:/usr/share/nginx/html/branding:ro
```

To use a different directory, set `BRANDING_DIR` in your `.env`:

```env
BRANDING_DIR=/path/to/my/assets
```

### Local Development

For `bun run dev`, create a symlink so Vite's dev server can serve the branding files:

```bash
cd frontend/public
ln -s ../../branding branding
```

## Color System

### Accent Color

`ACCENT_COLOR` sets the primary interactive color used for links, buttons, focus rings, and active indicators throughout the UI.

### Background Colors

Each theme (dark and light) takes a single base color. The frontend automatically derives a full surface palette from it:

- **5 surface shades** (from darkest to lightest for dark mode, reversed for light mode)
- **Border color**
- **Text hierarchy** (primary, secondary, muted, subtle, faint)

This means you only need to set one color per theme to get a cohesive palette.

### Success and Error Colors

`SUCCESS_COLOR` and `ERROR_COLOR` control status badges and indicators. For example, "Success" transaction badges use the success color, and "Failed" badges use the error color.

## Examples

### Blue theme

```env
CHAIN_NAME=MegaChain
CHAIN_LOGO_URL=/branding/logo.png
ACCENT_COLOR=#3b82f6
BACKGROUND_COLOR_DARK=#0a0a1a
BACKGROUND_COLOR_LIGHT=#e6f0f4
```

### Green theme (Eden)

```env
CHAIN_NAME=Eden
CHAIN_LOGO_URL=/branding/logo.svg
ACCENT_COLOR=#4ade80
BACKGROUND_COLOR_DARK=#0a1f0a
BACKGROUND_COLOR_LIGHT=#e8f5e8
SUCCESS_COLOR=#22c55e
ERROR_COLOR=#dc2626
```

### Minimal — just rename

```env
CHAIN_NAME=MyChain
```

Everything else stays default Atlas branding.

## How It Works

1. The backend reads branding env vars at startup and serves them via `GET /api/config`
2. The frontend fetches this config once on page load
3. CSS custom properties are set on the document root, overriding the defaults
4. Background surface shades are derived automatically using HSL color manipulation
5. The page title, navbar logo, and favicon are updated dynamically

No frontend rebuild is needed — just change the env vars and restart the API.
