# Product and Roadmap

This document describes the current Atlas product state and the remaining roadmap. It replaces the older MVP checklist, which is now mostly implemented.

## Product Positioning

Atlas is a lightweight EVM explorer for one ev-node based chain per deployment. It prioritizes operational simplicity, high-throughput indexing, useful public APIs, and white-label deployment over Blockscout-scale breadth.

Primary audiences:

- Chain operators deploying an explorer for an ev-node based chain.
- Developers inspecting blocks, transactions, contracts, ERC-20 tokens, and NFTs.
- Community users browsing chain activity.

## Implemented Product Areas

### Core Explorer

- Blocks, block details, and block transaction lists.
- Transactions, transaction details, logs, ERC-20 transfers, and NFT transfers.
- Address list and address detail pages.
- Universal search for chain entities and token names.
- Status dashboard with chain metadata, indexed height, totals, and charts.
- Live block updates through SSE with polling fallback.

### Indexing

- Batch block and receipt fetching from EVM JSON-RPC.
- Configurable start block, batch size, fetch workers, RPC batch size, and RPC rate limit.
- Binary COPY bulk writes for high indexing throughput.
- Resume from persisted indexer state.
- Reindex mode for wiping indexed data and rebuilding.
- Failed-block recording and gap-fill recovery worker.

### Tokens and NFTs

- ERC-20 detection, contracts, balances, holders, transfers, and charts.
- NFT collection/token indexing, ownership, transfer history, and token detail pages.
- NFT metadata state machine with retryable and permanent error handling.
- IPFS/Arweave/data/HTTP metadata resolution with safety checks.
- Full NFT metadata display with raw JSON inspection.
- NFT token name search backed by trigram indexing.

### Contracts

- Solidity source verification via native API and frontend contract tab.
- solc compiler download/cache support.
- Verified source, ABI, compiler settings, and metadata storage.
- Transaction input decoding against available ABIs.
- Proxy detection and combined ABI exposure.

### Operations

- Single `atlas-server` binary for API, indexer, workers, and CLI utilities.
- Docker Compose deployment with `postgres`, `atlas-server`, and `atlas-frontend`.
- Health probes, Prometheus metrics, structured logging option, and request timeouts.
- Scheduled database snapshots and manual dump/restore/reset commands.
- Runtime white-label branding and optional faucet support.
- Optional DA inclusion tracking from ev-node.

## Current Non-Goals

- Multi-chain indexing in one deployment.
- Account systems, user preferences, or explorer login.
- ERC-1155 indexing.
- Full EVM trace/internal transaction indexing.
- Gas oracle service or mempool analytics.
- Browser-based contract write UI.
- Hosted image caching/proxying for NFT media.

## Roadmap Candidates

These are not committed interfaces; they are candidate future work.

| Area | Candidate |
| --- | --- |
| ERC-1155 | Index multi-token transfers, balances, metadata, and UI pages |
| Traces | Optional `debug_traceTransaction` or `trace_transaction` ingestion for internal calls |
| Labels | Native address label management API and import/export workflow |
| Contract interaction | Read-only contract calls from verified ABIs; later wallet-backed write flows |
| Search | Attribute search for NFTs and richer ranked text search |
| Media | Optional NFT image proxy/cache for unreliable upstream metadata |
| Operations | More deployment examples for Kubernetes and managed Postgres |
| API compatibility | Additional Etherscan-compatible actions where tools need them |

## Success Criteria

Atlas is healthy when:

- `atlas-server check` passes against the target DB and RPC endpoint.
- `/health/live` returns `200`.
- `/health/ready` returns `200` while the indexer is fresh.
- `/metrics` exposes Prometheus data.
- `/api/height` advances as blocks are indexed.
- The frontend can load `/api/config`, show the configured chain name, and browse blocks.
- Reindexing from `START_BLOCK=0` works on a fresh local chain.
- NFT metadata failures are visible as retryable or permanent states rather than silent missing data.

## Acceptance Baseline for New Features

New product features should include:

- Backend behavior documented in `docs/API.md` or `backend/README.md` when it changes public interfaces.
- Frontend behavior documented in `frontend/README.md` when it changes routes or runtime assumptions.
- Environment variables added to `.env.example` and the backend README.
- Unit tests for new Rust logic in the same file where practical.
- Integration tests for API behavior when the route or query semantics change.
- No use of large-table `OFFSET` pagination for block-scale data.
