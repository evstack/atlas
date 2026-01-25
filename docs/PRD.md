# Atlas - Lightweight Ethereum L2 Block Explorer

## Overview

Atlas is a minimal, fast blockchain explorer for custom Ethereum L2 networks. It replaces Blockscout with a focused feature set, trading breadth for simplicity and performance.

**Target users:** Development team, community members, general public.

---

## Goals

1. **Simplicity** - Lean codebase, minimal dependencies, easy to operate
2. **Performance** - Fast queries, responsive UI, efficient indexing
3. **Full history** - Complete chain indexing with reindex capability
4. **NFT-first** - Rich NFT metadata indexing and display

## Non-Goals

- Multi-chain support (single L2 only)
- ERC-1155 support (ERC-721 only)
- User accounts or authentication
- Smart contract IDE/debugging
- Gas price oracles or analytics dashboards

---

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   React     │────▶│ Rust API    │────▶│  Postgres   │
│   Frontend  │     │  (Axum)     │     │             │
└─────────────┘     └──────┬──────┘     └─────────────┘
                          │                    ▲
                          │                    │
                   ┌──────▼──────┐            │
                   │  Indexer    │────────────┘
                   │  (async)    │
                   └──────┬──────┘
                          │
                   ┌──────▼──────┐
                   │  L2 Node    │
                   │  (RPC)      │
                   └─────────────┘
```

### Components

| Component | Tech | Purpose |
|-----------|------|---------|
| **API Server** | Rust (Axum) | REST API, serves frontend |
| **Indexer** | Rust (tokio) | Polls RPC, indexes blocks/txs/NFTs |
| **Database** | PostgreSQL | Persistent storage, full-text search |
| **Frontend** | React | UI, can be SSR or SPA |
| **Metadata Fetcher** | Rust (async) | Resolves and caches NFT tokenURIs |

---

## Data Model

### Core Entities

#### Block

```
- number: u64 (PK)
- hash: bytes32
- parent_hash: bytes32
- timestamp: u64
- gas_used: u64
- gas_limit: u64
- transaction_count: u32
- indexed_at: timestamp
```

#### Transaction

```
- hash: bytes32 (PK)
- block_number: u64 (FK)
- block_index: u32
- from_address: address
- to_address: address (nullable, for contract creation)
- value: u256
- gas_price: u256
- gas_used: u64
- input_data: bytes
- status: bool
- contract_created: address (nullable)
- timestamp: u64
```

#### Address

```
- address: bytes20 (PK)
- is_contract: bool
- first_seen_block: u64
- tx_count: u32
- balance: u256 (optional, can be fetched live)
```

#### NFT Contract (ERC-721)

```
- address: bytes20 (PK)
- name: string
- symbol: string
- total_supply: u64 (if enumerable)
- first_seen_block: u64
```

#### NFT Token

```
- contract_address: bytes20 (PK)
- token_id: u256 (PK)
- owner: address
- token_uri: string
- metadata_fetched: bool
- metadata: jsonb (nullable)
- image_url: string (nullable)
- last_transfer_block: u64
```

#### NFT Transfer

```
- id: serial (PK)
- tx_hash: bytes32 (FK)
- log_index: u32
- contract_address: bytes20
- token_id: u256
- from_address: address
- to_address: address
- block_number: u64
- timestamp: u64
```

---

## Features

### P0 - Core (MVP)

#### Block Explorer

- [ ] View latest blocks with pagination
- [ ] Block detail page (hash, txs, gas, timestamp)
- [ ] Navigate between blocks (prev/next)

#### Transaction Explorer

- [ ] View transactions in a block
- [ ] Transaction detail page (hash, from, to, value, gas, input data, status)
- [ ] Decode common function selectors (transfer, approve, etc.)

#### Address Pages

- [ ] View address balance (live RPC call)
- [ ] List transactions for address (sent/received)
- [ ] Identify contract vs EOA
- [ ] List NFTs owned by address

#### NFT Explorer

- [ ] List all indexed ERC-721 contracts
- [ ] View NFT collection page (name, symbol, tokens)
- [ ] View individual NFT (image, metadata, owner, transfer history)
- [ ] NFT transfer history per token

#### Search

- [ ] Search by transaction hash
- [ ] Search by address
- [ ] Search by block number/hash
- [ ] Search by NFT token name (from metadata)

#### Indexer

- [ ] Index blocks from genesis or configured start block
- [ ] Index transactions and receipts
- [ ] Detect and index ERC-721 contracts (via Transfer events)
- [ ] Queue NFT metadata fetching
- [ ] Support reindex from scratch (wipe and rebuild)
- [ ] Resume from last indexed block on restart

### P1 - Enhanced

#### NFT Metadata

- [ ] Fetch and cache tokenURI metadata
- [ ] Handle IPFS URIs (via gateway)
- [ ] Handle HTTP URIs
- [ ] Store and display NFT attributes
- [ ] Retry failed metadata fetches

#### Search Enhancements

- [ ] Full-text search on NFT names/descriptions
- [ ] Search NFTs by attribute values

#### UI Polish

- [ ] Real-time block updates (websocket or polling)
- [ ] Copy-to-clipboard for addresses/hashes
- [ ] Mobile-responsive design
- [ ] Loading states and error handling

### P2 - Nice to Have

#### Contract Verification

- [ ] Upload and verify Solidity source
- [ ] Display verified source code
- [ ] Decode transaction input against verified ABI

#### API

- [ ] Documented REST API
- [ ] Rate limiting
- [ ] API usage stats

---

## Search Implementation

| Query Type | Detection | Implementation |
|------------|-----------|----------------|
| Tx hash | 66 chars, starts with 0x | Direct lookup |
| Address | 42 chars, starts with 0x | Direct lookup |
| Block number | Numeric | Direct lookup |
| Block hash | 66 chars, starts with 0x | Direct lookup |
| NFT/Token name | String | Postgres full-text search on metadata |

---

## Indexer Behavior

### Startup

1. Check last indexed block in DB
2. If reindex flag set, truncate tables and start from genesis/config
3. Otherwise, resume from last indexed block + 1

### Indexing Loop

1. Fetch block via `eth_getBlockByNumber` (with txs)
2. Fetch receipts via `eth_getBlockReceipts` (or individual)
3. Parse Transfer events for ERC-721 detection
4. Insert block, transactions, addresses, NFT data
5. Queue metadata fetch jobs for new NFTs
6. Commit transaction
7. Update last indexed block
8. Sleep or continue based on head distance

### Metadata Fetching

- Separate async task pool
- Fetch tokenURI from contract
- Resolve URI (IPFS via configurable gateway, HTTP direct)
- Parse JSON metadata
- Store in database
- Retry with exponential backoff on failure

---

## Configuration

```toml
[rpc]
url = "https://your-l2-rpc.example.com"
requests_per_second = 100  # rate limit

[database]
url = "postgres://user:pass@localhost/atlas"
max_connections = 20

[indexer]
start_block = 0  # or "genesis"
batch_size = 100
reindex = false

[metadata]
ipfs_gateway = "https://ipfs.io/ipfs/"
fetch_workers = 4
retry_attempts = 3

[server]
host = "0.0.0.0"
port = 3000
```

---

## API Endpoints (Draft)

### Blocks

- `GET /api/blocks` - List blocks (paginated)
- `GET /api/blocks/:number` - Block detail
- `GET /api/blocks/:number/transactions` - Transactions in block

### Transactions

- `GET /api/transactions/:hash` - Transaction detail

### Addresses

- `GET /api/addresses/:address` - Address detail
- `GET /api/addresses/:address/transactions` - Address transactions
- `GET /api/addresses/:address/nfts` - NFTs owned by address

### NFTs

- `GET /api/nfts/collections` - List NFT contracts
- `GET /api/nfts/collections/:address` - Collection detail
- `GET /api/nfts/collections/:address/tokens` - Tokens in collection
- `GET /api/nfts/collections/:address/tokens/:id` - Token detail
- `GET /api/nfts/collections/:address/tokens/:id/transfers` - Token transfers

### Search

- `GET /api/search?q=:query` - Universal search

---

## Tech Stack

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Language | Rust | Performance, safety, low resource usage |
| Web framework | Axum | Async, ergonomic, well-maintained |
| Database | PostgreSQL | Full-text search, JSONB, reliable |
| ORM/Query | SQLx | Compile-time checked queries, async |
| HTTP client | reqwest | Async, well-maintained |
| RPC | ethers-rs or alloy | Ethereum types and RPC |
| Frontend | React | Familiar, adequate for scope |
| Styling | Tailwind CSS | Fast iteration, no custom CSS overhead |

---

## Deployment

### Minimum Requirements

- 2 vCPU, 4GB RAM (indexer + API)
- PostgreSQL 14+
- Network access to L2 RPC

### Docker Compose (Development)

```yaml
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_DB: atlas
      POSTGRES_USER: atlas
      POSTGRES_PASSWORD: atlas
    volumes:
      - pgdata:/var/lib/postgresql/data
    ports:
      - "5432:5432"

  atlas:
    build: .
    environment:
      DATABASE_URL: postgres://atlas:atlas@postgres/atlas
      RPC_URL: ${RPC_URL}
    ports:
      - "3000:3000"
    depends_on:
      - postgres

volumes:
  pgdata:
```

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| RPC rate limiting | Slow indexing | Configurable rate limits, batch requests |
| Large metadata (images) | Storage bloat | Store URLs only, don't cache images |
| IPFS gateway unreliable | Missing metadata | Multiple gateway fallbacks, retry queue |
| Chain reorgs | Data inconsistency | Track finalized blocks, reorg handling |
| Full history = slow initial sync | Long bootstrap | Configurable start block, progress API |

---

## Open Questions

1. **Image caching** - Should we proxy/cache NFT images or just link to source?
   - Recommendation: Link to source initially, add caching later if needed

2. **Reorg handling** - How deep can reorgs be on your L2?
   - Need to know finality guarantees to set confirmation depth

3. **Token standards** - Any custom ERC-721 extensions in use?
   - May need custom event parsing

4. **Internal transactions** - Do you need to trace internal calls?
   - Requires debug/trace APIs, significantly more complex

---

## Timeline Estimate

Not providing time estimates per instructions. Work breakdown:

**Phase 1: Foundation**

- Database schema and migrations
- Indexer core (blocks, transactions)
- Basic API endpoints

**Phase 2: NFT Indexing**

- ERC-721 detection and indexing
- Metadata fetching pipeline
- NFT API endpoints

**Phase 3: Frontend**

- Block/transaction views
- Address pages
- NFT gallery and detail views
- Search

**Phase 4: Polish**

- Real-time updates
- Error handling
- Performance optimization
- Documentation

---

## Success Metrics

- Indexer keeps up with chain head (< 10 block lag)
- API p99 latency < 200ms
- Search returns results < 500ms
- 95%+ NFT metadata successfully fetched
- Zero data loss on restart/crash
