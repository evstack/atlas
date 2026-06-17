# Architecture

Atlas is a single-chain EVM explorer for ev-node based chains. The backend is one Rust process, `atlas-server`, that combines the indexer, HTTP API, live event stream, and background workers.

## System Overview

```text
                         JSON-RPC
                    +----------------+
                    | EVM / ev-node  |
                    +-------+--------+
                            |
                            v
+-------------------------------------------------------------------+
|                         atlas-server                              |
|                                                                   |
|  +--------------+       +----------------+       +-------------+  |
|  | Block fetch  | ----> | Batch builder  | ----> | Binary COPY |  |
|  | workers      |       | logs/tokens    |       | writer      |  |
|  +--------------+       +----------------+       +------+------+  |
|                                                           |        |
|  +----------------+   +----------------+   +-------------+-------+|
|  | Metadata       |   | Gap fill       |   | DA worker           ||
|  | worker         |   | worker         |   | optional ev-node    ||
|  +----------------+   +----------------+   +-------------+-------+|
|                                                           |        |
|  +----------------+   +----------------+   +-------------+-------+|
|  | Axum REST API  |   | SSE /api/events|   | Prometheus metrics ||
|  +----------------+   +----------------+   +---------------------+|
+-----------------------------+-------------------------------------+
                              |
                              v
                         +----------+
                         | Postgres |
                         +----------+
                              ^
                              |
                    +---------+----------+
                    | atlas-frontend     |
                    | nginx + Vite build |
                    +--------------------+
```

## Repository Layout

```text
atlas/
+-- backend/
|   +-- Cargo.toml
|   +-- crates/
|   |   +-- atlas-common/      # Shared models, errors, pagination, DB helpers
|   |   +-- atlas-server/      # API, indexer, CLI, workers, metrics
|   +-- migrations/            # SQLx migrations
+-- frontend/                  # Vite app, Bun, Tailwind, Preact compatibility
+-- branding/                  # Optional mounted logo/static branding assets
+-- docs/
+-- docker-compose.yml
+-- .env.example
```

## Backend Process

`atlas-server run` starts:

- A migration pass before serving traffic.
- Separate SQLx pools for API and indexer work.
- The block fetch pipeline with configurable batch size, fetch workers, RPC batch size, and RPC rate limit.
- A binary COPY writer using a direct `tokio-postgres` connection.
- An Axum router with REST endpoints, Etherscan-compatible API, metrics, and health probes.
- An in-process `HeadTracker` used by `/api/height` and `/api/events`.
- The missed-block gap-fill worker.
- The NFT metadata worker.
- Optional DA tracking, faucet backend, and snapshot scheduler.

The `check` subcommand validates configuration and DB/RPC connectivity. The `migrate` subcommand runs migrations and exits. The `db` subcommands provide dump, restore, and indexed-data reset operations.

## Indexing Flow

1. The indexer resumes from stored state or `START_BLOCK`.
2. Fetch workers pull blocks and receipts from the configured EVM JSON-RPC endpoint.
3. The batch builder extracts blocks, transactions, addresses, event logs, ERC-20 transfers/balances, NFT transfers/ownership, contract metadata, and failed block state.
4. The writer persists batches with binary COPY and ordinary SQL where appropriate.
5. Committed batches update the `HeadTracker` and publish live block events.
6. Failed blocks are recorded for the gap-fill worker, which retries with backoff and clears recovered rows atomically.

The fresh-chain block-zero path is supported; recent underflow fixes ensure initial local-chain indexing works from `START_BLOCK=0`.

## Background Workers

### NFT Metadata

NFT tokens move through a metadata state machine:

- `pending`
- `fetched`
- `retryable_error`
- `permanent_error`

The worker resolves IPFS, Arweave, `data:` URI, direct image, and HTTP metadata references, applies SSRF and payload protections, classifies failures, stores retry timestamps, and exposes metadata status through NFT APIs.

### Data Availability Tracking

When `ENABLE_DA_TRACKING=true`, Atlas queries ev-node using `EVNODE_URL` for Celestia header and data inclusion heights. The worker backfills known blocks, updates pending entries, stores results in `block_da_status`, and emits `da_batch` SSE events for visible UI updates.

### Snapshots

When `SNAPSHOT_ENABLED=true`, a scheduler runs daily `pg_dump` snapshots at `SNAPSHOT_TIME` UTC, writes custom-format dump files into `SNAPSHOT_DIR`, and keeps the newest `SNAPSHOT_RETENTION` completed dumps.

## API Layer

The API is built with Axum. Most routes have:

- 10 second HTTP timeout returning `408`.
- API pool `statement_timeout = '10s'`.
- HTTP request metrics collected by route.
- CORS allowing all origins by default or a single `CORS_ORIGIN` when configured.

Routes excluded from the 10 second HTTP timeout:

- `/api/events`, because SSE connections are long-lived.
- `/api/contracts/{address}/verify`, because Solidity compilation can take longer and accepts up to 50 MiB request bodies.

## Database

PostgreSQL stores canonical explorer history and derived views. Large tables are optimized for explorer access patterns:

- Blocks avoid large `OFFSET` scans by using a cursor derived from the highest indexed block and the clamped page limit.
- Large table counts use `pg_class.reltuples` estimates where appropriate, with exact counts for small tables.
- Transactions use lookup tables and partition-aware queries where needed.
- Migrations run with a dedicated one-connection pool that is not constrained by the API statement timeout.

## Frontend and Deployment

The frontend is a Vite build served by unprivileged nginx:

- Container port `8080`, exposed as host port `80` in Docker Compose.
- `/api/` and exact `/api` are proxied to `atlas-server:3000`.
- `/api/events` is proxied with buffering disabled.
- `/branding/` serves mounted branding assets.
- SPA routes fall back to `index.html`.

In development, Vite serves on `5173` and proxies browser `/api/...` requests to `localhost:3000`.

## Observability

- `/metrics` exposes Prometheus text format.
- `/health/live` reports process liveness.
- `/health/ready` checks DB connectivity and recent indexer state.
- Logs support text or JSON format through `LOG_FORMAT`.
- Metrics include indexer head, missing block state, HTTP route metrics, and SSE connection accounting.
