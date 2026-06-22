# Atlas

Atlas is an EVM blockchain explorer for ev-node based chains. It runs a Rust indexer, REST API, background workers, PostgreSQL database, and Vite frontend as one deployable stack.

## Current Architecture

- `atlas-server`: one Rust binary that runs the indexer, API, live SSE stream, metrics endpoint, optional faucet, optional Data Availability tracking, NFT metadata worker, missed-block gap-fill worker, and optional snapshot scheduler.
- `postgres`: stores indexed blocks, transactions, addresses, ERC-20 data, NFT data, logs, contract verification data, proxy metadata, and indexer state.
- `atlas-frontend`: Vite app served by unprivileged nginx on port 8080 inside the container, exposed as host port 80 by Docker Compose.

## Quick Start

Requirements:

- Docker and Docker Compose
- An EVM JSON-RPC endpoint

```bash
cp .env.example .env
# Edit RPC_URL and CHAIN_NAME in .env
docker compose up -d
```

Default service URLs:

| Service | URL |
| --- | --- |
| Explorer UI | `http://localhost` |
| API through frontend nginx | `http://localhost/api` |
| Backend API when running `atlas-server` locally | `http://localhost:3000/api` |
| Prometheus metrics when backend port is reachable | `http://localhost:3000/metrics` |
| Liveness probe when backend port is reachable | `http://localhost:3000/health/live` |
| Readiness probe when backend port is reachable | `http://localhost:3000/health/ready` |

The `postgres` service binds to `127.0.0.1:5432` for local development access. Docker Compose does not publish `atlas-server:3000` to the host by default; the frontend proxies browser API traffic through `http://localhost/api`.

## Local Development

Start only PostgreSQL:

```bash
docker compose up -d postgres
```

Run the backend:

```bash
cd backend
export DATABASE_URL=postgres://atlas:atlas@localhost:5432/atlas
export RPC_URL=http://localhost:8545
cargo run --bin atlas-server -- run
```

Run the frontend:

```bash
cd frontend
bun install
bun run dev
```

The Vite dev server runs on `http://localhost:5173` and proxies `/api` to `http://localhost:3000`.

## Operator Commands

Run these from `backend/` unless noted otherwise:

```bash
cargo run --bin atlas-server -- check
cargo run --bin atlas-server -- migrate
cargo run --bin atlas-server -- run
cargo run --bin atlas-server -- db dump /tmp/atlas.dump
cargo run --bin atlas-server -- db restore /tmp/atlas.dump
cargo run --bin atlas-server -- db reset --confirm
```

Useful validation commands:

```bash
cd backend && cargo test --workspace
cd frontend && bun run build
cd frontend && bun run lint
```

## Configuration

Copy `.env.example` to `.env`. `RPC_URL` is required. When running without Docker, `DATABASE_URL` is also required.

Common variables:

| Variable | Purpose | Default |
| --- | --- | --- |
| `RPC_URL` | EVM JSON-RPC endpoint | required |
| `DATABASE_URL` | PostgreSQL connection URL for local backend runs | required outside Docker |
| `CHAIN_NAME` | Display name served by `/api/config` and `/api/status` | `Unknown` |
| `START_BLOCK` | First block to index | `0` |
| `BATCH_SIZE` | Blocks written per DB batch | `100` |
| `FETCH_WORKERS` | Concurrent block fetch workers | `10` |
| `RPC_BATCH_SIZE` | Blocks fetched per RPC batch call | `20` |
| `RPC_REQUESTS_PER_SECOND` | RPC request rate limit | `100` |
| `DB_MAX_CONNECTIONS` | Indexer pool size | `20` |
| `API_DB_MAX_CONNECTIONS` | API pool size | `20` |
| `IPFS_GATEWAY` | Gateway for IPFS NFT metadata | `https://ipfs.io/ipfs/` |
| `METADATA_FETCH_WORKERS` | NFT metadata worker concurrency | `4` |
| `METADATA_RETRY_ATTEMPTS` | Retry attempts for retryable metadata failures | `3` |
| `REINDEX` | Wipe indexed data and restart from `START_BLOCK` | `false` |

Optional features include Data Availability tracking, faucet support, white-label branding, scheduled snapshots, JSON logging, CORS restriction, and a Solidity compiler cache. See [.env.example](.env.example) and [Backend](backend/README.md) for the full operator reference.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [API Reference](docs/API.md)
- [Features](docs/FEATURES.md)
- [White Labeling](docs/WHITE_LABELING.md)
- [Product and Roadmap](docs/PRD.md)
- [Backend](backend/README.md)
- [Frontend](frontend/README.md)

## License

MIT
