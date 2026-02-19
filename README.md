# Atlas

A lightweight Ethereum L2 blockchain explorer.

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Bun 1.0+ (for frontend development)
- Rust 1.75+ (for backend development)

### Running with Docker

```bash
cp .env.example .env
# Edit .env with your RPC endpoint

docker-compose up -d
```

Access the explorer at http://localhost:3000

### Local Development

**Backend:**

```bash
cd backend

# Start PostgreSQL
docker-compose up -d postgres

# Set environment
export DATABASE_URL=postgres://atlas:atlas@localhost/atlas
export RPC_URL=https://your-l2-rpc.example.com

# Run services
cargo run --bin atlas-indexer
cargo run --bin atlas-api  # in another terminal
```

**Frontend:**

```bash
cd frontend
bun install
bun run dev
```

## Configuration

Copy `.env.example` to `.env` and set your RPC endpoint. Available options:

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
