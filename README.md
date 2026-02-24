# Atlas

A lightweight Ethereum L2 blockchain explorer.

## Quick Start

### Prerequisites

- `just` 1.0+
- Docker and Docker Compose
- Bun 1.0+
- Rust 1.75+

### Running with Docker

```bash
cp .env.example .env
docker-compose up -d
```

Access the explorer at http://localhost:3000

### Local Development

```bash
cp .env.example .env
docker-compose up -d postgres
just frontend-install
```

Start backend services (each in its own terminal):

```bash
just backend-indexer
```

```bash
just backend-api
```

Start frontend:

```bash
just frontend-dev
```

### Useful Commands

```bash
just --list
just frontend-lint
just frontend-build
just backend-fmt
just backend-clippy
just backend-test
just ci
```

## Configuration

Copy `.env.example` to `.env` and set `RPC_URL`. Common options:

| Variable | Description | Default |
|----------|-------------|---------|
| `RPC_URL` | Ethereum JSON-RPC endpoint | Required |
| `DATABASE_URL` | PostgreSQL connection string | Set in docker-compose |
| `START_BLOCK` | Block to start indexing from | `0` |
| `BATCH_SIZE` | Blocks per indexing batch | `100` |
| `RPC_REQUESTS_PER_SECOND` | RPC rate limit | `100` |
| `FETCH_WORKERS` | Parallel block fetch workers | `10` |
| `RPC_BATCH_SIZE` | Blocks per RPC batch request | `20` |
| `IPFS_GATEWAY` | Gateway for NFT metadata | `https://ipfs.io/ipfs/` |
| `REINDEX` | Wipe and reindex from start | `false` |

## Documentation

- [API Reference](docs/API.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Product Requirements](docs/PRD.md)

## License

MIT
