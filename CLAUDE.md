# Atlas — Claude Code Context

Atlas is an EVM blockchain explorer (indexer + API + frontend) for ev-node based chains.

## Tech Stack

| Layer | Tech |
|---|---|
| Indexer | Rust, tokio, sqlx, alloy, tokio-postgres (binary COPY) |
| API | Rust, Axum, sqlx, tower-http |
| Database | PostgreSQL (partitioned tables) |
| Frontend | React, TypeScript, Vite, Tailwind CSS, Bun |
| Deployment | Docker Compose, nginx (unprivileged, port 8080→80) |

## Repository Layout

```
atlas/
├── backend/
│   ├── Cargo.toml                  # Workspace — all dep versions live here
│   ├── crates/
│   │   ├── atlas-common/           # Shared types, DB pool, error handling, Pagination
│   │   ├── atlas-indexer/          # Block fetcher, batch writer, metadata fetcher
│   │   └── atlas-api/              # Axum REST API
│   └── migrations/                 # sqlx migrations (run at startup by both crates)
├── frontend/
│   ├── src/
│   │   ├── api/                    # Typed API clients (axios)
│   │   ├── components/             # Shared UI components
│   │   ├── hooks/                  # React hooks (useBlocks, useLatestBlockHeight, …)
│   │   ├── pages/                  # One file per page/route
│   │   └── types/                  # Shared TypeScript types
│   ├── Dockerfile                  # Multi-stage: oven/bun:1 → nginx-unprivileged:alpine
│   └── nginx.conf                  # SPA routing + /api/ reverse proxy to atlas-api:3000
├── docker-compose.yml
└── .env.example
```

## Key Architectural Decisions

### Database connection pools
- **API pool**: 20 connections, `statement_timeout = '10s'` set via `after_connect` hook
- **Indexer pool**: 20 connections (configurable via `DB_MAX_CONNECTIONS`), same timeout
- **Binary COPY client**: separate `tokio-postgres` direct connection (bypasses sqlx pool), conditional TLS based on `sslmode` in DATABASE_URL
- **Migrations**: run with a dedicated 1-connection pool with **no** statement_timeout (index builds can take longer than 10s)

### Pagination — blocks table
The blocks table can have 80M+ rows. `OFFSET` on large pages causes 30s+ full index scans. Instead:
```rust
// cursor = max_block - (page - 1) * limit  — uses clamped limit(), not raw offset()
let limit = pagination.limit();  // clamped to 100
let cursor = (total_count - 1) - (pagination.page.saturating_sub(1) as i64) * limit;
// Query: WHERE number <= $cursor ORDER BY number DESC LIMIT $1
```
`total_count` comes from `MAX(number) + 1` (O(1), not COUNT(*)).

### Row count estimation
For large tables (transactions, addresses), use `pg_class.reltuples` instead of `COUNT(*)`:
```rust
// handlers/mod.rs — get_table_count(pool, "table_name")
// Partition-aware: sums child reltuples, falls back to parent
// For tables < 100k rows: falls back to exact COUNT(*)
```

### HTTP timeout
`TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(10))` wraps all routes — returns 408 if any handler exceeds 10s.

### AppState (API)
```rust
pub struct AppState {
    pub pool: PgPool,
    pub rpc_url: String,
    pub solc_path: String,
    pub admin_api_key: Option<String>,
    pub chain_id: u64,       // fetched from RPC once at startup via eth_chainId
    pub chain_name: String,  // from CHAIN_NAME env var, defaults to "Unknown"
}
```

### Frontend API client
- Base URL: `/api` (proxied by nginx to `atlas-api:3000`)
- Fast polling endpoint: `GET /api/height` → `{ block_height, indexed_at }` — used by navbar every 2s
- Chain status: `GET /api/status` → full chain info, fetched once on page load

## Important Conventions

- **Rust**: idiomatic — use `.min()`, `.max()`, `|=`, `+=` over manual if/assign
- **SQL**: never use `OFFSET` for large tables — use keyset/cursor pagination
- **Migrations**: use `run_migrations(&database_url)` (not `&pool`) to get a timeout-free connection
- **Frontend**: uses Bun (not npm/yarn). Lockfile is `bun.lock` (text, Bun ≥ 1.2). Build with `bunx vite build` (skips tsc type check).
- **Docker**: frontend image uses `nginxinc/nginx-unprivileged:alpine` (non-root, port 8080). API/indexer use `alpine` with `ca-certificates`.
- **Commits**: authored by the user only — no Claude co-author lines.

## Environment Variables

Key vars (see `.env.example` for full list):

| Var | Used by | Default |
|---|---|---|
| `DATABASE_URL` | all | required |
| `RPC_URL` | indexer, api | required |
| `CHAIN_NAME` | api | `"Unknown"` |
| `DB_MAX_CONNECTIONS` | indexer | `20` |
| `BATCH_SIZE` | indexer | `100` |
| `FETCH_WORKERS` | indexer | `10` |
| `ADMIN_API_KEY` | api | none |

## Running Locally

```bash
# Start full stack
docker compose up -d

# Rebuild a single service after code changes
docker compose build atlas-api && docker compose up -d atlas-api

# Backend only (no Docker)
cd backend && cargo build --workspace
```

## Common Gotchas

- `get_table_count(pool, table_name)` — pass the table name, it's not hardcoded anymore
- `run_migrations` takes `&str` (database URL), not `&PgPool`
- The blocks cursor uses `pagination.limit()` (clamped), not `pagination.offset()` — they diverge when client sends `limit > 100`
- `bun.lock` not `bun.lockb` — Bun ≥ 1.2 uses text format
