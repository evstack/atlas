# Atlas Backend

Rust backend services for the Atlas blockchain explorer.

## Crates

### atlas-common

Shared types and utilities:
- Database models with SQLx `FromRow` derives
- Error types
- Pagination helpers
- Database connection pool

### atlas-indexer

Blockchain indexer that:
- Connects to L2 RPC endpoint
- Indexes blocks and transactions
- Detects ERC-721 Transfer events
- Manages NFT ownership tracking
- Fetches and caches NFT metadata

### atlas-api

REST API server (Axum) providing:
- Block/transaction/address endpoints
- NFT collection and token endpoints
- ERC-20 token endpoints
- Event log endpoints
- Address labels
- Proxy contract detection
- Contract verification
- Etherscan-compatible API
- Universal search
- Health check endpoint

## Building

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run checks
cargo check

# Run tests
cargo test

# Run clippy
cargo clippy
```

## Running

```bash
# Required environment variables
export DATABASE_URL=postgres://atlas:atlas@localhost/atlas
export RPC_URL=https://your-l2-rpc.example.com

# Run indexer
cargo run --bin atlas-indexer

# Run API server
cargo run --bin atlas-api
```

## Project Structure

```
backend/
├── Cargo.toml           # Workspace definition
├── Dockerfile           # Multi-stage Docker build
├── migrations/          # SQLx migrations
│   └── *.sql
└── crates/
    ├── atlas-common/
    │   └── src/
    │       ├── lib.rs
    │       ├── types.rs   # Database models
    │       ├── error.rs   # Error types
    │       └── db.rs      # Connection pool
    ├── atlas-indexer/
    │   └── src/
    │       ├── main.rs
    │       ├── config.rs
    │       ├── indexer.rs
    │       └── metadata.rs
    └── atlas-api/
        └── src/
            ├── main.rs
            ├── error.rs
            └── handlers/
                ├── mod.rs
                ├── blocks.rs
                ├── transactions.rs
                ├── addresses.rs
                ├── nfts.rs
                ├── tokens.rs
                ├── logs.rs
                ├── labels.rs
                ├── proxy.rs
                ├── contracts.rs
                ├── etherscan.rs
                └── search.rs
```

## Dependencies

Key dependencies:
- `tokio` - Async runtime
- `axum` - Web framework
- `sqlx` - Async PostgreSQL client
- `alloy` - Ethereum types and RPC
- `reqwest` - HTTP client for metadata fetching
- `tracing` - Structured logging

## Database Migrations

Migrations run automatically on startup. To run manually:

```bash
# Install sqlx-cli
cargo install sqlx-cli

# Run migrations
sqlx migrate run
```
