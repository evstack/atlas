# API Reference

Base URL: `http://localhost:3000`

## Pagination

All list endpoints support pagination:

| Parameter | Default | Max | Description |
|-----------|---------|-----|-------------|
| `page` | 1 | - | Page number |
| `limit` | 20 | 100 | Items per page |

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

## Endpoints

### Status

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/height` | Current block height and indexer timestamp (lightweight, safe to poll frequently) |
| GET | `/api/status` | Full chain status: chain ID, chain name, block height, total transactions, total addresses |
| GET | `/health` | Health check (returns "OK") |

**`/api/status` response:**
```json
{
  "chain_id": 1,
  "chain_name": "My Chain",
  "block_height": 1000000,
  "total_transactions": 5000000,
  "total_addresses": 200000,
  "indexed_at": "2026-01-01T00:00:00+00:00"
}
```
`chain_name` is set via the `CHAIN_NAME` environment variable.

### Blocks

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/blocks` | List blocks (newest first) |
| GET | `/api/blocks/:number` | Get block by number |
| GET | `/api/blocks/:number/transactions` | Get transactions in block |

### Transactions

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/transactions` | List transactions (newest first) |
| GET | `/api/transactions/:hash` | Get transaction details |
| GET | `/api/transactions/:hash/logs` | Get event logs |
| GET | `/api/transactions/:hash/logs/decoded` | Get decoded event logs with signatures |
| GET | `/api/transactions/:hash/erc20-transfers` | Get ERC-20 transfers in transaction |
| GET | `/api/transactions/:hash/nft-transfers` | Get NFT transfers in transaction |

### Addresses

| Method | Path | Parameters | Description |
|--------|------|------------|-------------|
| GET | `/api/addresses` | `is_contract`, `from_block`, `to_block`, `address_type` | List addresses |
| GET | `/api/addresses/:address` | - | Get address details |
| GET | `/api/addresses/:address/transactions` | - | Get address transactions |
| GET | `/api/addresses/:address/transfers` | `transfer_type` (erc20/nft) | Get all transfers |
| GET | `/api/addresses/:address/nfts` | - | Get NFTs owned |
| GET | `/api/addresses/:address/tokens` | - | Get ERC-20 balances |
| GET | `/api/addresses/:address/logs` | `topic0` | Get event logs |
| GET | `/api/addresses/:address/label` | - | Get address with label |

**Address Types**: `eoa`, `contract`, `erc20`, `nft`

### NFT Collections

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/nfts/collections` | List NFT collections |
| GET | `/api/nfts/collections/:address` | Get collection details |
| GET | `/api/nfts/collections/:address/tokens` | List tokens in collection |
| GET | `/api/nfts/collections/:address/transfers` | Get collection transfers |
| GET | `/api/nfts/collections/:address/tokens/:token_id` | Get token details |
| GET | `/api/nfts/collections/:address/tokens/:token_id/transfers` | Get token transfer history |

### ERC-20 Tokens

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/tokens` | List ERC-20 tokens |
| GET | `/api/tokens/:address` | Get token details (includes holder/transfer counts) |
| GET | `/api/tokens/:address/holders` | Get token holders with balances |
| GET | `/api/tokens/:address/transfers` | Get token transfers |

### Event Logs

| Method | Path | Parameters | Description |
|--------|------|------------|-------------|
| GET | `/api/logs` | `topic0` (required) | Filter logs by event signature |

### Address Labels

| Method | Path | Parameters | Description |
|--------|------|------------|-------------|
| GET | `/api/labels` | `tag`, `search` | List labels |
| GET | `/api/labels/:address` | - | Get label for address |
| GET | `/api/labels/tags` | - | Get all tags with counts |
| POST | `/api/labels` | Body: `{address, name, tags[]}` | Create/update label |
| POST | `/api/labels/bulk` | Body: `{labels: [...]}` | Bulk import labels |
| DELETE | `/api/labels/:address` | - | Delete label |

### Contract Verification

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/contracts/:address/abi` | Get verified ABI |
| GET | `/api/contracts/:address/source` | Get verified source code |
| POST | `/api/contracts/verify` | Verify contract source |

**Verification Body:**
```json
{
  "address": "0x...",
  "source_code": "...",
  "contract_name": "MyContract",
  "compiler_version": "v0.8.19+commit.7dd6d404",
  "optimization_enabled": true,
  "optimization_runs": 200,
  "constructor_args": "0x...",
  "evm_version": "paris",
  "license_type": "MIT",
  "is_standard_json": false
}
```

### Proxy Contracts

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/proxies` | List detected proxy contracts |
| GET | `/api/contracts/:address/proxy` | Get proxy info |
| GET | `/api/contracts/:address/combined-abi` | Get merged proxy + implementation ABI |
| POST | `/api/contracts/:address/detect-proxy` | Trigger proxy detection |

**Proxy Types**: `eip1967`, `eip1822`, `transparent`, `custom`

### Search

| Method | Path | Parameters | Description |
|--------|------|------------|-------------|
| GET | `/api/search` | `q` (required) | Universal search |

Searches across:
- Block numbers
- Transaction hashes
- Addresses
- Contract/token names

## Etherscan-Compatible API

For tooling compatibility, the following Etherscan-style endpoints are supported:

### Account Module

```
GET /api?module=account&action=balance&address=0x...
GET /api?module=account&action=balancemulti&address=0x...,0x...
GET /api?module=account&action=txlist&address=0x...
GET /api?module=account&action=txlistinternal&address=0x...
GET /api?module=account&action=tokentx&address=0x...
GET /api?module=account&action=tokenbalance&address=0x...&contractaddress=0x...
```

### Contract Module

```
GET /api?module=contract&action=getabi&address=0x...
GET /api?module=contract&action=getsourcecode&address=0x...
POST /api?module=contract&action=verifysourcecode
```

### Transaction Module

```
GET /api?module=transaction&action=gettxreceiptstatus&txhash=0x...
```

### Block Module

```
GET /api?module=block&action=getblockreward&blockno=123
```

### Proxy Module (RPC)

```
GET /api?module=proxy&action=eth_blockNumber
GET /api?module=proxy&action=eth_getBlockByNumber&tag=0x...&boolean=true
GET /api?module=proxy&action=eth_getTransactionByHash&txhash=0x...
```

## Notes

- All address parameters accept with or without `0x` prefix
- Addresses are case-insensitive (normalized to lowercase)
- Transaction hashes accept with or without `0x` prefix
