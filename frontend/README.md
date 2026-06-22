# Atlas Frontend

The frontend is a Vite app written with React-compatible components and bundled through Preact compatibility aliases. It is built with Bun and served in production by `nginxinc/nginx-unprivileged:alpine`.

## Tech Stack

- Vite 7
- TypeScript
- Preact via `@preact/preset-vite` and `preact/compat` aliases for React imports
- React Router
- Tailwind CSS
- Recharts
- Bun

## Development

```bash
bun install
bun run dev
bun run build
bun run lint
bun run preview
```

The dev server listens on `http://localhost:5173` and proxies `/api` to `http://localhost:3000`, so browser requests still use the `/api/...` paths.

## API Base URL

The API client reads:

```env
VITE_API_BASE_URL=http://localhost:3000/api
```

If unset, development defaults to `http://localhost:3000/api`. The Docker production build sets `VITE_API_BASE_URL=/api`, so browser requests go through nginx to `atlas-server:3000`.

## Production nginx

The frontend image:

- Builds with `bunx vite build`
- Serves static assets on container port `8080`
- Uses SPA fallback routing through `index.html`
- Proxies `/api/` and exact `/api` to `atlas-server:3000`
- Proxies `/api/events` separately with buffering disabled for SSE
- Allows contract verification uploads up to `50m`
- Serves mounted branding assets from `/branding/`

Docker Compose exposes the frontend as host port `80:8080`.

## Routes

Current app routes:

| Route | Page |
| --- | --- |
| `/` | Welcome dashboard |
| `/blocks` | Blocks list with live updates and optional DA status |
| `/blocks/:number` | Block details |
| `/blocks/:number/transactions` | Block transactions |
| `/transactions` | Transactions list |
| `/tx/:hash` | Transaction details, logs, token/NFT transfers, decoded input where ABI is available |
| `/addresses` | Address list |
| `/address/:address` | Address details, transactions, tokens, NFTs, transfers, contract tab |
| `/tokens` | ERC-20 token list |
| `/tokens/:address` | Token details, holders, transfers, charts |
| `/nfts` | NFT collection list |
| `/nfts/:contract` | NFT collection tokens and transfers |
| `/nfts/:contract/:tokenId` | NFT token detail, transfer history, full metadata, raw JSON toggle |
| `/status` | Chain status and charts |
| `/faucet` | Faucet page, feature-gated by `/api/config` |
| `/search` | Search results |

## Runtime Data Flow

- `BrandingProvider` fetches `/api/config` once at startup and applies chain name, logos, theme colors, feature flags, and faucet metadata.
- The layout uses `/api/events` for live block updates and `/api/height` as a lightweight polling fallback.
- Pages use typed API wrappers in `src/api`.
- NFT views use metadata status fields to distinguish pending, fetched, retryable error, and permanent error states.
- DA UI is shown only when `/api/config.features.da_tracking` is true.

## Branding Assets

In Docker, `${BRANDING_DIR:-./branding}` is mounted to `/usr/share/nginx/html/branding:ro`.

For Vite development, expose the same directory through `frontend/public`:

```bash
mkdir -p branding
cd frontend/public
ln -s ../../branding branding
```

Then set paths such as `CHAIN_LOGO_URL=/branding/logo.svg`.

## Project Structure

```text
frontend/
+-- src/
|   +-- api/          # Typed fetch clients
|   +-- components/   # Shared UI components
|   +-- context/      # Theme, branding, live block stats
|   +-- hooks/        # Data-fetching and SSE hooks
|   +-- pages/        # Route components
|   +-- types/        # Shared TypeScript API types
|   +-- utils/        # Formatting, ABI decode, metadata helpers
+-- nginx.conf
+-- Dockerfile
+-- package.json
+-- vite.config.ts
```
