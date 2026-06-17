# Atlas Backend

The backend is a Rust workspace containing `atlas-common` and the combined `atlas-server` binary. `atlas-server` runs the HTTP API, chain indexer, live update stream, background workers, and operator utilities.

## Crates

| Crate | Purpose |
| --- | --- |
| `atlas-common` | Shared SQLx models, error types, pagination, and database helpers |
| `atlas-server` | Axum API, indexer, CLI, metrics, faucet, snapshots, DA tracking, NFT metadata, and database utilities |

There are no separate backend service binaries for the API and indexer. Use `atlas-server` for all backend operations.

## Commands

From `backend/`:

```bash
# Validate configuration and DB/RPC connectivity
cargo run --bin atlas-server -- check

# Run migrations and exit
cargo run --bin atlas-server -- migrate

# Start API, indexer, and background workers
cargo run --bin atlas-server -- run

# Database utilities
cargo run --bin atlas-server -- db dump /tmp/atlas.dump
cargo run --bin atlas-server -- db restore /tmp/atlas.dump
cargo run --bin atlas-server -- db reset --confirm
```

`db dump` uses `pg_dump --format=custom --no-owner --no-acl`. `db restore` resets the public schema and runs `pg_restore --exit-on-error`. `pg_dump`, `pg_restore`, and `psql` must be available on the host for these utilities.

## Required Environment

When running outside Docker:

```bash
export DATABASE_URL=postgres://atlas:atlas@localhost:5432/atlas
export RPC_URL=http://localhost:8545
cargo run --bin atlas-server -- run
```

Docker Compose sets `DATABASE_URL` for the `atlas-server` container. `RPC_URL` must be provided in `.env`.

## CLI and Environment Reference

Most runtime options are available as environment variables and CLI flags.

| Area | Env var | CLI flag | Default |
| --- | --- | --- | --- |
| Database | `DB_MAX_CONNECTIONS` | `--atlas.db.max-connections` | `20` |
| Database | `API_DB_MAX_CONNECTIONS` | `--atlas.db.api-max-connections` | `20` |
| RPC | `RPC_URL` | `--atlas.rpc.url` | required |
| RPC | `RPC_REQUESTS_PER_SECOND` | `--atlas.rpc.requests-per-second` | `100` |
| RPC | `RPC_BATCH_SIZE` | `--atlas.rpc.batch-size` | `20` |
| API | `API_HOST` | `--atlas.api.host` | `127.0.0.1` |
| API | `API_PORT` | `--atlas.api.port` | `3000` |
| API | `CORS_ORIGIN` | `--atlas.api.cors-origin` | allow all |
| API | `SSE_REPLAY_BUFFER_BLOCKS` | `--atlas.api.sse-replay-buffer-blocks` | `4096` |
| Contracts | `SOLC_CACHE_DIR` | `--atlas.api.solc-cache-dir` | `/tmp/solc-cache` |
| Indexer | `START_BLOCK` | `--atlas.indexer.start-block` | `0` |
| Indexer | `BATCH_SIZE` | `--atlas.indexer.batch-size` | `100` |
| Indexer | `FETCH_WORKERS` | `--atlas.indexer.fetch-workers` | `10` |
| Indexer | `REINDEX` | `--atlas.indexer.reindex` | `false` |
| Indexer | `IPFS_GATEWAY` | `--atlas.indexer.ipfs-gateway` | `https://ipfs.io/ipfs/` |
| NFT metadata | `METADATA_FETCH_WORKERS` | `--atlas.indexer.metadata-fetch-workers` | `4` |
| NFT metadata | `METADATA_RETRY_ATTEMPTS` | `--atlas.indexer.metadata-retry-attempts` | `3` |
| Chain | `CHAIN_NAME` | `--atlas.chain.name` | `Unknown` |
| Chain | `CHAIN_LOGO_URL` | `--atlas.chain.logo-url` | unset |
| Chain | `CHAIN_LOGO_URL_LIGHT` | `--atlas.chain.logo-url-light` | unset |
| Chain | `CHAIN_LOGO_URL_DARK` | `--atlas.chain.logo-url-dark` | unset |
| DA tracking | `ENABLE_DA_TRACKING` | `--atlas.da.enabled` | `false` |
| DA tracking | `EVNODE_URL` | `--atlas.da.evnode-url` | required when enabled |
| DA tracking | `DA_WORKER_CONCURRENCY` | `--atlas.da.worker-concurrency` | `50` |
| DA tracking | `DA_RPC_REQUESTS_PER_SECOND` | `--atlas.da.rpc-requests-per-second` | `50` |
| Faucet | `FAUCET_ENABLED` | `--atlas.faucet.enabled` | `false` |
| Faucet | `FAUCET_AMOUNT` | `--atlas.faucet.amount` | required when enabled |
| Faucet | `FAUCET_COOLDOWN_MINUTES` | `--atlas.faucet.cooldown-minutes` | required when enabled |
| Logging | `RUST_LOG` | `--atlas.log.level` | `atlas_server=info,tower_http=debug,sqlx=warn` |
| Logging | `LOG_FORMAT` | `--atlas.log.format` | `text` |

`FAUCET_PRIVATE_KEY` is env-only by design and must be set when `FAUCET_ENABLED=true`.

Snapshot settings are env-only:

| Env var | Purpose | Default |
| --- | --- | --- |
| `SNAPSHOT_ENABLED` | Enable daily `pg_dump` snapshots | `false` |
| `SNAPSHOT_TIME` | UTC time in `HH:MM` format | `03:00` |
| `SNAPSHOT_RETENTION` | Number of completed snapshot files to keep | `7` |
| `SNAPSHOT_DIR` | Container path for snapshot files | `/snapshots` |
| `SNAPSHOT_HOST_DIR` | Host bind mount used by Docker Compose | `./snapshots` |

## Runtime Behavior

- Migrations run on startup through a dedicated migration connection without the API query timeout.
- The API and indexer use separate SQLx pools. API connections apply a 10 second `statement_timeout`; the indexer pool uses the configured max connection count.
- Bulk writes use a separate `tokio-postgres` binary COPY connection so high-volume indexing does not consume the API pool.
- The indexer fetches blocks in batches, writes blocks/transactions/logs/token data, and publishes committed heads to the in-process `HeadTracker`.
- The gap-fill worker retries rows in `failed_blocks` with backoff and clears recovered failures atomically.
- The NFT metadata worker processes `pending` and retryable tokens, resolves IPFS/Arweave/data/HTTP URIs, classifies failures as retryable or permanent, and stores status fields on `nft_tokens`.
- Optional DA tracking queries ev-node for Celestia header/data inclusion heights and publishes `da_batch` SSE events.
- Optional snapshots write portable custom-format dumps named `atlas_snapshot_<timestamp>.dump`.

## API and Probes

The Axum router exposes the REST API under `/api`, Prometheus metrics at `/metrics`, and health endpoints:

| Path | Purpose |
| --- | --- |
| `/health` | Legacy plain-text OK response |
| `/health/live` | Process liveness JSON |
| `/health/ready` | DB connectivity plus fresh indexer state |
| `/metrics` | Prometheus text format |

Most routes are wrapped in a 10 second HTTP timeout and return `408 Request Timeout` when exceeded. `/api/events` and `/api/contracts/{address}/verify` are excluded because SSE streams stay open and Solidity compilation can take longer.

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

Integration tests live under `backend/crates/atlas-server/tests/integration` and use testcontainers and wiremock where needed.
