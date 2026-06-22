# White Labeling

Atlas branding is configured at runtime through `atlas-server` environment variables and served to the frontend by `GET /api/config`. A frontend rebuild is not required for normal branding changes.

## Runtime Config

| Variable | Description | Default |
| --- | --- | --- |
| `CHAIN_NAME` | Displayed in page title, navbar, welcome page, and `/api/status` | `Unknown` |
| `CHAIN_LOGO_URL` | Default logo URL or path | bundled logo |
| `CHAIN_LOGO_URL_LIGHT` | Logo used in light theme | falls back to `CHAIN_LOGO_URL` |
| `CHAIN_LOGO_URL_DARK` | Logo used in dark theme | falls back to `CHAIN_LOGO_URL` |
| `ACCENT_COLOR` | Primary interactive color | `#dc2626` |
| `BACKGROUND_COLOR_DARK` | Dark theme base background | `#050505` |
| `BACKGROUND_COLOR_LIGHT` | Light theme base background | `#f4ede6` |
| `SUCCESS_COLOR` | Success state color | `#22c55e` |
| `ERROR_COLOR` | Error state color | `#dc2626` |

Example:

```env
CHAIN_NAME=My Chain
CHAIN_LOGO_URL=/branding/logo.svg
CHAIN_LOGO_URL_LIGHT=/branding/logo-light.svg
CHAIN_LOGO_URL_DARK=/branding/logo-dark.svg
ACCENT_COLOR=#3b82f6
BACKGROUND_COLOR_DARK=#070a12
BACKGROUND_COLOR_LIGHT=#f5f7fb
SUCCESS_COLOR=#22c55e
ERROR_COLOR=#ef4444
```

## `/api/config`

The backend returns branding, feature flags, and faucet display metadata:

```json
{
  "chain_name": "My Chain",
  "logo_url": "/branding/logo.svg",
  "logo_url_light": "/branding/logo-light.svg",
  "logo_url_dark": "/branding/logo-dark.svg",
  "accent_color": "#3b82f6",
  "background_color_dark": "#070a12",
  "background_color_light": "#f5f7fb",
  "success_color": "#22c55e",
  "error_color": "#ef4444",
  "features": {
    "da_tracking": false
  },
  "faucet": {
    "enabled": false
  }
}
```

Unset optional fields are omitted. The frontend normalizes this response, applies CSS custom properties, updates the favicon/title, and gates optional UI such as DA indicators and faucet navigation.

## Branding Assets

Place assets in the repository-level `branding/` directory:

```text
atlas/
+-- branding/
|   +-- logo.svg
|   +-- logo-light.svg
|   +-- logo-dark.svg
+-- docker-compose.yml
+-- .env
```

Set URLs relative to the frontend origin:

```env
CHAIN_LOGO_URL=/branding/logo.svg
CHAIN_LOGO_URL_LIGHT=/branding/logo-light.svg
CHAIN_LOGO_URL_DARK=/branding/logo-dark.svg
```

## Docker

Docker Compose mounts branding assets into the frontend container:

```yaml
atlas-frontend:
  volumes:
    - ${BRANDING_DIR:-./branding}:/usr/share/nginx/html/branding:ro
```

To use another host directory:

```env
BRANDING_DIR=/path/to/branding-assets
```

Restart the frontend container after changing mounted files if nginx or browser cache prevents immediate refresh.

## Local Vite Development

Expose repository branding assets through Vite's public directory:

```bash
mkdir -p branding
cd frontend/public
ln -s ../../branding branding
```

Run the backend with the branding env vars set, then start Vite:

```bash
cd backend
cargo run --bin atlas-server -- run

cd ../frontend
bun run dev
```

## Color Behavior

- `ACCENT_COLOR` drives links, active states, focus rings, and primary buttons.
- `BACKGROUND_COLOR_DARK` and `BACKGROUND_COLOR_LIGHT` are base colors; the frontend derives related surface, border, and text colors.
- `SUCCESS_COLOR` and `ERROR_COLOR` drive status badges and indicators.
- Theme-specific logos let deployers use different marks for light and dark backgrounds.

Use valid CSS hex colors. Invalid or empty values are ignored by the frontend normalization layer.

## Deployment Checklist

- Set `CHAIN_NAME`.
- Add logo assets to `branding/` or use absolute hosted image URLs.
- Set `CHAIN_LOGO_URL_LIGHT` and `CHAIN_LOGO_URL_DARK` when one logo does not work in both themes.
- Set accent/background colors and verify both light and dark mode.
- Confirm `GET /api/config` returns expected values.
- Open the frontend and verify navbar, welcome page, favicon, charts, buttons, badges, and optional feature-gated nav items.
