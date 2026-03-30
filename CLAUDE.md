# Atlas — Claude Code Context

Atlas is an EVM blockchain explorer (indexer + API + frontend) for ev-node based chains.

## Tech Stack

| Layer | Tech |
|---|---|
| Server | Rust, tokio, Axum, sqlx, alloy, tokio-postgres (binary COPY), tower-http |
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
│   │   └── atlas-server/           # Unified server: indexer + API in a single binary
│   │       └── src/
│   │           ├── main.rs          # Startup: migrations, pools, spawn indexer, serve API
│   │           ├── config.rs        # Unified config from env vars
│   │           ├── indexer/         # Block fetcher, batch writer, metadata fetcher
│   │           └── api/             # Axum REST API + SSE handlers
│   └── migrations/                 # sqlx migrations (run once at startup)
├── frontend/
│   ├── src/
│   │   ├── api/                    # Typed API clients (axios)
│   │   ├── components/             # Shared UI components
│   │   ├── hooks/                  # React hooks (useBlocks, useLatestBlockHeight, …)
│   │   ├── pages/                  # One file per page/route
│   │   └── types/                  # Shared TypeScript types
│   ├── Dockerfile                  # Multi-stage: oven/bun:1 → nginx-unprivileged:alpine
│   └── nginx.conf                  # SPA routing + /api/ reverse proxy to atlas-server:3000
├── docker-compose.yml
└── .env.example
```

## Key Architectural Decisions

### Single binary
The indexer and API run as concurrent tokio tasks in a single `atlas-server` binary. The indexer pushes block events directly to SSE subscribers via an in-process `broadcast::Sender<()>`. If the indexer task fails, the API keeps running (graceful degradation); the indexer retries with exponential backoff.

### Database connection pools
- **API pool**: 20 connections (configurable via `API_DB_MAX_CONNECTIONS`), `statement_timeout = '10s'`
- **Indexer pool**: 20 connections (configurable via `DB_MAX_CONNECTIONS`), same timeout — kept separate so API load can't starve the indexer
- **Binary COPY client**: separate `tokio-postgres` direct connection (bypasses sqlx pool), conditional TLS based on `sslmode` in DATABASE_URL
- **Migrations**: run once with a dedicated 1-connection pool with **no** statement_timeout (index builds can take longer than 10s)

### SSE live updates
The indexer publishes block updates through `broadcast::Sender<()>`. SSE handler (`GET /api/events`) subscribes to this broadcast channel and refreshes independently of the database write path.

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
// handlers/mod.rs — get_table_count(pool, table_name)
// Partition-aware: sums child reltuples, falls back to parent
// For tables < 100k rows: falls back to exact COUNT(*)
```

### HTTP timeout
`TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(10))` wraps all routes except SSE — returns 408 if any handler exceeds 10s.

### AppState (API)
```rust
pub struct AppState {
    pub pool: PgPool,                                // API pool only
    pub block_events_tx: broadcast::Sender<()>,      // shared with indexer
    pub da_events_tx: broadcast::Sender<Vec<DaSseUpdate>>, // shared with DA worker
    pub head_tracker: Arc<HeadTracker>,
    pub rpc_url: String,
    pub da_tracking_enabled: bool,
    pub chain_id: u64,
    pub chain_name: String,
}
```

### DA tracking (optional)
When `ENABLE_DA_TRACKING=true`, a background DA worker queries ev-node for Celestia inclusion heights per block. `EVNODE_URL` is required only in that mode. Updates are pushed to SSE clients via an in-process `broadcast::Sender<Vec<DaSseUpdate>>`. The SSE handler streams `da_batch` events for incremental updates and emits `da_resync` when a client falls behind and should refetch visible DA state.

### S3 archive (optional)
When `S3_ENABLED=true`, a background `ArchiveUploader` task uploads every indexed block to an S3-compatible object store as a zstd-compressed JSON bundle (block header + receipts). The write path uses a transactional outbox pattern:
1. The indexer writes archive entries to `archive_blocks` (with compressed payload) inside the same DB transaction as the block data.
2. The uploader claims rows via `SELECT … FOR UPDATE SKIP LOCKED`, uploads to S3, then clears the payload column.
3. `archive_state` tracks the latest *contiguous* uploaded block and triggers a manifest refresh at `{prefix}/v1/manifest.json` so consumers know how far the archive has progressed.

Object key layout: `{prefix}/v1/blocks/{bucket_start:012}/{block_number:012}.json.zst` where `bucket_start = (block_number / 10_000) * 10_000`.

Works with AWS S3 and any S3-compatible store (MinIO, Cloudflare R2, etc.). The URL is constructed automatically from `S3_BUCKET` + `S3_REGION` + optional `S3_ENDPOINT` — there is no single URL config. Set `S3_FORCE_PATH_STYLE=true` for MinIO and other self-hosted stores; leave it `false` for AWS.

### Frontend API client
- Base URL: `/api` (proxied by nginx to `atlas-server:3000`)
- Fast polling endpoint: `GET /api/height` → `{ block_height, indexed_at, features: { da_tracking } }` — serves from `head_tracker` first and falls back to `indexer_state` when the in-memory head is empty. Used by the navbar as a polling fallback when SSE is disconnected and by feature-flag consumers.
- Chain status: `GET /api/status` → `{ chain_id, chain_name, block_height, total_transactions, total_addresses, indexed_at }` — full chain info, fetched once on page load.
- `GET /api/events` → SSE stream of `new_block`, `da_batch`, and `da_resync` events. Primary live-update path for navbar counter, blocks page, block detail DA status, and DA resync handling. Falls back to `/api/height` polling on disconnect.

## Important Conventions

- **Rust**: idiomatic — use `.min()`, `.max()`, `|=`, `+=` over manual if/assign
- **SQL**: never use `OFFSET` for large tables — use keyset/cursor pagination
- **Migrations**: use `run_migrations(&database_url)` (not `&pool`) to get a timeout-free connection
- **Frontend**: uses Bun (not npm/yarn). Lockfile is `bun.lock` (text, Bun ≥ 1.2). Build with `bunx vite build` (skips tsc type check).
- **Docker**: frontend image uses `nginxinc/nginx-unprivileged:alpine` (non-root, port 8080). Server uses `alpine` with `ca-certificates`.
- **Tests**: add unit tests for new logic in a `#[cfg(test)] mod tests` block in the same file. Run with `cargo test --workspace`.
- **Commits**: authored by the user only — no Claude co-author lines.

## Environment Variables

Key vars (see `.env.example` for full list):

| Var | Used by | Default |
|---|---|---|
| `DATABASE_URL` | all | required |
| `RPC_URL` | server | required |
| `CHAIN_NAME` | server | `"Unknown"` |
| `DB_MAX_CONNECTIONS` | indexer pool | `20` |
| `API_DB_MAX_CONNECTIONS` | API pool | `20` |
| `BATCH_SIZE` | indexer | `100` |
| `FETCH_WORKERS` | indexer | `10` |
| `ADMIN_API_KEY` | API | none |
| `API_HOST` | API | `127.0.0.1` |
| `API_PORT` | API | `3000` |
| `ENABLE_DA_TRACKING` | server | `false` |
| `EVNODE_URL` | server | none |
| `DA_RPC_REQUESTS_PER_SECOND` | DA worker | `50` |
| `DA_WORKER_CONCURRENCY` | DA worker | `50` |
| `S3_ENABLED` | archive | `false` |
| `S3_BUCKET` | archive | required if enabled |
| `S3_REGION` | archive | required if enabled |
| `S3_PREFIX` | archive | `""` (bucket root) |
| `S3_ENDPOINT` | archive | none (AWS S3) |
| `S3_FORCE_PATH_STYLE` | archive | `false` |
| `S3_UPLOAD_CONCURRENCY` | archive | `4` |
| `S3_RETRY_BASE_SECONDS` | archive | `30` |
| `AWS_ACCESS_KEY_ID` | archive | env / IAM role |
| `AWS_SECRET_ACCESS_KEY` | archive | env / IAM role |

## Running Locally

```bash
# Start full stack
docker compose up -d

# Rebuild after code changes
docker compose build atlas-server && docker compose up -d atlas-server

# Backend only (no Docker)
cd backend && cargo build --workspace
```

## Common Gotchas

- `run_migrations` takes `&str` (database URL), not `&PgPool`
- The blocks cursor uses `pagination.limit()` (clamped), not `pagination.offset()` — they diverge when client sends `limit > 100`
- `bun.lock` not `bun.lockb` — Bun ≥ 1.2 uses text format
- SSE uses in-process broadcast, not PG NOTIFY — no PgListener needed
