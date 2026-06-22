# Features

This document describes the implemented Atlas feature set on `origin/main` through merged PR #79.

## Explorer Core

- Latest blocks and block details, including base fee, gas, transaction count, and optional DA status.
- Transaction lists and transaction details with status, value, gas, input data, transfers, and logs.
- Address pages with transaction history, ERC-20 balances, NFT ownership, transfers, and a contract tab for verified contracts.
- Universal search for block numbers, block hashes, transaction hashes, addresses, NFT collections, NFT token names, and ERC-20 names/symbols.
- Status page with chain ID, chain name, indexed height, estimated totals, and charts.
- Live block updates through Server-Sent Events with `/api/height` polling fallback.

## Tokens and NFTs

### ERC-20

- Detects ERC-20 transfers from event logs.
- Stores token contracts, transfers, balances, holders, and indexed supply history where available.
- Exposes token list, token detail, holders, transfer history, address balances, transaction token transfers, and token chart data.
- Recomputes indexed total supply from balances when complete history is available.

### NFTs

- Detects NFT contracts and transfers.
- Tracks token ownership and transfer history.
- Fetches and stores token metadata, image URL, name, full JSON metadata, retry status, and error state.
- Shows pending/unavailable metadata states in the frontend.
- NFT token pages display common fields and provide a raw JSON toggle for full metadata inspection.
- NFT token name search uses trigram indexing for fast text lookup.

## Event Logs and Decoding

- Indexes raw event logs for transactions and emitting addresses.
- Provides transaction logs and decoded transaction logs.
- Uses known event signatures and verified ABIs where available.
- Supports address log pagination with explicit `page` and `limit` query parsing.

## Contract Verification and Proxies

- Verifies Solidity contracts through `POST /api/contracts/{address}/verify`.
- Supports standard JSON input and single-source verification.
- Downloads and caches solc binaries in `SOLC_CACHE_DIR`.
- Stores source, compiler settings, ABI, bytecode metadata, and verification status.
- Displays verified source and ABI in the frontend contract tab.
- Decodes transaction input against verified ABIs where possible.
- Detects proxy contracts and exposes proxy info plus combined proxy/implementation ABI.

## Etherscan-Compatible API

Atlas supports Etherscan-style responses at exact `/api` for tooling compatibility:

- `account`: balance, multi-balance, tx list, internal tx placeholder, token transfers, token balance.
- `contract`: get ABI, get source code.
- `transaction`: receipt status.
- `block`: block reward.
- `proxy`: selected RPC passthrough actions.

The native contract verification route is `/api/contracts/{address}/verify`; the current Etherscan-compatible contract module does not implement `verifysourcecode`.

## Live Updates

- `/api/events` streams committed indexed blocks as `new_block` events.
- When DA tracking is enabled, the same stream emits `da_batch` updates.
- The frontend buffers and de-duplicates live blocks, then falls back to polling when SSE disconnects.
- `SSE_REPLAY_BUFFER_BLOCKS` controls the in-memory block replay buffer for connected clients.

## Data Availability Tracking

Optional DA tracking uses ev-node to record Celestia inclusion heights:

- Enable with `ENABLE_DA_TRACKING=true`.
- Configure ev-node with `EVNODE_URL`.
- Tune with `DA_WORKER_CONCURRENCY` and `DA_RPC_REQUESTS_PER_SECOND`.
- API and frontend expose header/data DA heights for blocks.
- DA UI is feature-gated by `/api/config.features.da_tracking`.

## Faucet

Optional faucet support:

- Enable with `FAUCET_ENABLED=true`.
- Configure `FAUCET_PRIVATE_KEY`, `FAUCET_AMOUNT`, and `FAUCET_COOLDOWN_MINUTES`.
- `/api/config` exposes whether the faucet is enabled and its public amount/cooldown metadata.
- `/api/faucet/info` returns faucet balance and settings.
- `/api/faucet` sends funds and returns a transaction hash.
- Cooldown failures return `429` with retry metadata.

## White Labeling

Branding is runtime configurable through environment variables:

- Chain name.
- Default, light-theme, and dark-theme logos.
- Accent, background, success, and error colors.
- Mounted `/branding/` assets in Docker.
- `/api/config` as the frontend runtime source of truth.

No frontend rebuild is required for branding changes when using runtime environment configuration and mounted assets.

## Operations

- `atlas-server check` validates DB/RPC configuration.
- `atlas-server migrate` runs SQLx migrations.
- `atlas-server run` starts API, indexer, and workers.
- `atlas-server db dump` creates portable `pg_dump` custom-format backups.
- `atlas-server db restore` restores backups after resetting the public schema.
- `atlas-server db reset --confirm` truncates indexed data while preserving schema and migrations.
- Scheduled snapshots can run daily through env-only `SNAPSHOT_*` settings.

## Observability and Reliability

- Prometheus metrics at `/metrics`.
- Liveness and readiness probes at `/health/live` and `/health/ready`.
- HTTP route metrics with matched route labels.
- 10 second API timeout and database statement timeout for request-serving pools.
- Timeout-free migration pool for long index builds.
- Missed-block gap-fill worker with retry/backoff.
- RPC request rate limiting and batch fetching.
- Binary COPY batch writes for high-throughput indexing.

## Current Non-Goals and Gaps

- Multi-chain indexing in one Atlas deployment.
- User accounts or per-user saved settings.
- ERC-1155 indexing.
- Full trace/internal transaction indexing.
- Browser-based contract write interactions.
- Native label management API routes.
