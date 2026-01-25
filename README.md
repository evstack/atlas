# Atlas

A lightweight Ethereum L2 blockchain explorer with NFT support.

## Overview

Atlas is a minimal, fast blockchain explorer designed for custom Ethereum L2 networks. It provides:

- Block and transaction browsing
- Address tracking with labels
- ERC-721 NFT indexing with metadata
- ERC-20 token indexing and balances
- Event log decoding
- Contract verification
- Proxy contract detection
- Etherscan-compatible API
- Universal search

## Architecture

```
atlas/
├── backend/           # Rust backend services
│   ├── crates/
│   │   ├── atlas-common/    # Shared types and database models
│   │   ├── atlas-indexer/   # Blockchain indexer + metadata fetcher
│   │   └── atlas-api/       # REST API server (Axum)
│   └── migrations/          # PostgreSQL migrations
├── frontend/          # React frontend (Vite + Tailwind)
├── docker-compose.yml # Container orchestration
└── docs/
    └── PRD.md         # Product requirements
```

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Bun 1.0+ (for frontend development)
- Rust 1.75+ (for backend development)

### Running with Docker

1. Set your RPC endpoint:

   ```bash
   export RPC_URL=https://your-l2-rpc.example.com
   ```

2. Start all services:

   ```bash
   docker-compose up -d
   ```

3. Access the explorer at <http://localhost:3000>

### Local Development

**Backend:**

```bash
cd backend

# Start PostgreSQL
docker-compose up -d postgres

# Set environment variables
export DATABASE_URL=postgres://atlas:atlas@localhost/atlas
export RPC_URL=https://your-l2-rpc.example.com

# Run the indexer
cargo run --bin atlas-indexer

# In another terminal, run the API server
cargo run --bin atlas-api
```

**Frontend:**

```bash
cd frontend
bun install
bun run dev
```

## Configuration

### Environment Variables

| Variable                  | Description                         | Default                 |
|---------------------------|-------------------------------------|-------------------------|
| `DATABASE_URL`            | PostgreSQL connection string        | Required                |
| `RPC_URL`                 | Ethereum JSON-RPC endpoint          | Required                |
| `START_BLOCK`             | Block number to start indexing from | `0`                     |
| `BATCH_SIZE`              | Number of blocks to index per batch | `100`                   |
| `REINDEX`                 | Set to `true` to wipe and reindex   | `false`                 |
| `RPC_REQUESTS_PER_SECOND` | Rate limit for RPC calls            | `100`                   |
| `IPFS_GATEWAY`            | IPFS gateway for NFT metadata       | `https://ipfs.io/ipfs/` |
| `METADATA_FETCH_WORKERS`  | Concurrent metadata fetch workers   | `4`                     |
| `API_HOST`                | API server bind address             | `0.0.0.0`               |
| `API_PORT`                | API server port                     | `3000`                  |
| `SOLC_PATH`               | Path to solc binary for verification| `solc`                  |

## API Reference

### Blocks

| Endpoint                               | Description               |
|----------------------------------------|---------------------------|
| `GET /api/blocks`                      | List blocks (paginated)   |
| `GET /api/blocks/:number`              | Get block by number       |
| `GET /api/blocks/:number/transactions` | Get transactions in block |

### Transactions

| Endpoint                      | Description             |
|-------------------------------|-------------------------|
| `GET /api/transactions/:hash` | Get transaction by hash |

### Addresses

| Endpoint                                   | Description               |
|--------------------------------------------|---------------------------|
| `GET /api/addresses/:address`              | Get address info          |
| `GET /api/addresses/:address/transactions` | Get address transactions  |
| `GET /api/addresses/:address/nfts`         | Get NFTs owned by address |

### NFTs

| Endpoint                                                  | Description                |
|-----------------------------------------------------------|----------------------------|
| `GET /api/nfts/collections`                               | List NFT collections       |
| `GET /api/nfts/collections/:address`                      | Get collection details     |
| `GET /api/nfts/collections/:address/tokens`               | List tokens in collection  |
| `GET /api/nfts/collections/:address/tokens/:id`           | Get token details          |
| `GET /api/nfts/collections/:address/tokens/:id/transfers` | Get token transfer history |

### Tokens (ERC-20)

| Endpoint                              | Description                    |
|---------------------------------------|--------------------------------|
| `GET /api/tokens`                     | List ERC-20 tokens             |
| `GET /api/tokens/:address`            | Get token details              |
| `GET /api/tokens/:address/holders`    | Get token holders              |
| `GET /api/tokens/:address/transfers`  | Get token transfers            |
| `GET /api/addresses/:address/tokens`  | Get address token balances     |

### Event Logs

| Endpoint                                | Description                    |
|-----------------------------------------|--------------------------------|
| `GET /api/transactions/:hash/logs`      | Get transaction logs           |
| `GET /api/transactions/:hash/logs/decoded` | Get decoded transaction logs |
| `GET /api/addresses/:address/logs`      | Get logs emitted by contract   |
| `GET /api/logs?topic0=:sig`             | Filter logs by event signature |

### Address Labels

| Endpoint                   | Description                    |
|----------------------------|--------------------------------|
| `GET /api/labels`          | List address labels            |
| `GET /api/labels/:address` | Get label for address          |
| `GET /api/labels/tags`     | List all tags                  |
| `POST /api/labels`         | Create/update label            |
| `DELETE /api/labels/:address` | Delete label                |

### Proxy Contracts

| Endpoint                                  | Description                    |
|-------------------------------------------|--------------------------------|
| `GET /api/proxies`                        | List proxy contracts           |
| `GET /api/contracts/:address/proxy`       | Get proxy info                 |
| `GET /api/contracts/:address/combined-abi`| Get combined ABI               |
| `POST /api/contracts/:address/detect-proxy` | Trigger proxy detection      |

### Contract Verification

| Endpoint                          | Description                    |
|-----------------------------------|--------------------------------|
| `POST /api/contracts/verify`      | Submit source for verification |
| `GET /api/contracts/:address/abi` | Get verified ABI               |
| `GET /api/contracts/:address/source` | Get verified source code    |

### Etherscan-Compatible API

| Endpoint                                              | Description                    |
|-------------------------------------------------------|--------------------------------|
| `GET /api?module=account&action=balance`              | Get address balance            |
| `GET /api?module=account&action=txlist`               | Get address transactions       |
| `GET /api?module=account&action=tokentx`              | Get token transfers            |
| `GET /api?module=contract&action=getabi`              | Get contract ABI               |
| `GET /api?module=contract&action=getsourcecode`       | Get contract source            |
| `POST /api?module=contract&action=verifysourcecode`   | Verify contract source         |

### Search

| Endpoint                   | Description                                     |
|----------------------------|-------------------------------------------------|
| `GET /api/search?q=:query` | Universal search (blocks, txs, addresses, NFTs) |

### Pagination

All list endpoints support pagination:

- `page` - Page number (default: 1)
- `limit` - Items per page (default: 20, max: 100)

Response format:

```json
{
  "data": [...],
  "page": 1,
  "limit": 20,
  "total": 1000,
  "total_pages": 50
}
```

## Database Schema

### Tables

- `blocks` - Block headers
- `transactions` - Transaction data
- `addresses` - Known addresses
- `nft_contracts` - ERC-721 contract registry
- `nft_tokens` - NFT token ownership and metadata
- `nft_transfers` - NFT transfer history
- `erc20_contracts` - ERC-20 token registry
- `erc20_transfers` - ERC-20 transfer events
- `erc20_balances` - Token balances per address
- `event_logs` - All emitted events
- `event_signatures` - Known event signatures for decoding
- `address_labels` - Curated address labels
- `contract_abis` - Verified contract ABIs and source
- `indexer_state` - Indexer progress tracking

## Development

### Running Tests

```bash
cd backend
cargo test
```

### Building for Production

```bash
cd backend
cargo build --release
```

Binaries will be at:

- `backend/target/release/atlas-indexer`
- `backend/target/release/atlas-api`

### Frontend Build

```bash
cd frontend
bun run build
```

Build output will be in `frontend/dist/`.

## Reindexing

To reindex the chain from scratch:

```bash
# Option 1: Environment variable
REINDEX=true docker-compose up atlas-indexer

# Option 2: Truncate tables manually
psql $DATABASE_URL -c "TRUNCATE blocks, transactions, addresses, nft_contracts, nft_tokens, nft_transfers, indexer_state CASCADE;"
```

## License

MIT
