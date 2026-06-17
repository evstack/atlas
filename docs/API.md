# API Reference

Atlas exposes native REST endpoints under `/api`, an Etherscan-compatible query API at exact `/api`, Prometheus metrics at `/metrics`, and health probes under `/health`.

When using Docker Compose through the frontend, use base URL `/api`. When running the backend directly, use `http://localhost:3000/api`.

## Shared Behavior

### Pagination

List endpoints use:

| Parameter | Default | Max | Description |
| --- | --- | --- | --- |
| `page` | `1` | none | One-based page number |
| `limit` | `20` | `100` | Items per page |

Paginated response:

```json
{
  "data": [],
  "page": 1,
  "limit": 20,
  "total": 0,
  "total_pages": 0
}
```

Blocks use keyset-style pagination internally to avoid large `OFFSET` scans. Other list endpoints may still use offset pagination.

### Timeouts and Errors

Most routes are wrapped in a 10 second HTTP timeout and return `408 Request Timeout` if exceeded. `/api/events` and `/api/contracts/{address}/verify` are excluded because they can legitimately run longer.

Application errors are returned as JSON with an `error` field. Rate-limited faucet responses include a retry hint through `retry-after` and/or `retry_after_seconds`.

Timeout-generated `408` responses come from the HTTP timeout middleware, not the application error handler, so they return an empty body rather than the JSON error envelope.

### Address and Hash Parameters

Address and transaction hash parameters accept values with or without `0x` where the handler normalizes them. Addresses are stored lowercase.

## Status, Config, Health, Metrics

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/height` | Lightweight indexed-head endpoint for frequent polling |
| GET | `/api/status` | Chain ID, chain name, indexed height, and estimated totals |
| GET | `/api/config` | Branding, feature flags, and faucet config |
| GET | `/metrics` | Prometheus text format |
| GET | `/health` | Legacy plain-text `OK` |
| GET | `/health/live` | Liveness JSON |
| GET | `/health/ready` | Readiness JSON: DB reachable and indexer state fresh |

`GET /api/height`:

```json
{
  "block_height": 1000000,
  "indexed_at": "2026-01-01T00:00:00+00:00"
}
```

`GET /api/status`:

```json
{
  "chain_id": "31337",
  "chain_name": "Devnet",
  "block_height": 1000000,
  "total_transactions": 5000000,
  "total_addresses": 250000,
  "indexed_at": "2026-01-01T00:00:00+00:00"
}
```

`GET /api/config`:

```json
{
  "chain_name": "Devnet",
  "logo_url": "/branding/logo.svg",
  "logo_url_light": "/branding/logo-light.svg",
  "logo_url_dark": "/branding/logo-dark.svg",
  "accent_color": "#3b82f6",
  "background_color_dark": "#050505",
  "background_color_light": "#f4ede6",
  "success_color": "#22c55e",
  "error_color": "#dc2626",
  "features": { "da_tracking": true },
  "faucet": {
    "enabled": true,
    "amount_wei": "10000000000000000",
    "cooldown_minutes": 30
  }
}
```

Unset optional branding fields are omitted.

## Live Events

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/events` | Server-Sent Events stream for committed block heads and DA updates |

The stream is excluded from the 10 second timeout and has nginx buffering disabled in the production frontend image.

Event `new_block`:

```json
{
  "block": {
    "number": 1000000,
    "hash": "0x...",
    "parent_hash": "0x...",
    "timestamp": 1700000000,
    "gas_used": 21000,
    "gas_limit": 30000000,
    "base_fee_per_gas": "1000000000",
    "transaction_count": 1,
    "indexed_at": "2026-01-01T00:00:00+00:00"
  }
}
```

Event `da_batch`:

```json
{
  "updates": [
    {
      "block_number": 1000000,
      "header_da_height": 12345,
      "data_da_height": 12346
    }
  ]
}
```

Event `da_resync`:

```json
{
  "required": true
}
```

This event is emitted when a client falls behind the DA update stream and should refetch visible DA state instead of relying on incremental updates.

The stream represents committed indexed data, not speculative node head data. Clients should use `/api/blocks` and `/api/height` for catch-up and polling fallback.

## Blocks

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/blocks` | List blocks newest first |
| GET | `/api/blocks/{number}` | Get one block |
| GET | `/api/blocks/{number}/transactions` | List transactions in a block |

Block objects include `base_fee_per_gas` and, when DA tracking has data for that block, `da_status`.

## Transactions

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/transactions` | List transactions newest first |
| GET | `/api/transactions/{hash}` | Get transaction details |
| GET | `/api/transactions/{hash}/logs` | Get raw event logs |
| GET | `/api/transactions/{hash}/logs/decoded` | Get decoded event logs when signatures or verified ABIs are available |
| GET | `/api/transactions/{hash}/erc20-transfers` | ERC-20 transfers in a transaction |
| GET | `/api/transactions/{hash}/nft-transfers` | NFT transfers in a transaction |

## Addresses

| Method | Path | Query | Description |
| --- | --- | --- | --- |
| GET | `/api/addresses` | `is_contract`, `from_block`, `to_block`, `address_type` | List addresses |
| GET | `/api/addresses/{address}` | none | Address details |
| GET | `/api/addresses/{address}/transactions` | pagination | Address transaction history |
| GET | `/api/addresses/{address}/transfers` | `transfer_type=erc20|nft`, pagination | Token and NFT transfers |
| GET | `/api/addresses/{address}/nfts` | pagination | NFTs owned by address |
| GET | `/api/addresses/{address}/tokens` | pagination | ERC-20 balances |
| GET | `/api/addresses/{address}/logs` | `topic0`, pagination | Logs emitted by address |

`address_type` can identify EOAs, contracts, ERC-20 tokens, and NFT contracts where indexed data is available.

## ERC-20 Tokens

| Method | Path | Query | Description |
| --- | --- | --- | --- |
| GET | `/api/tokens` | pagination | List ERC-20 tokens |
| GET | `/api/tokens/{address}` | none | Token detail with holder and transfer counts |
| GET | `/api/tokens/{address}/holders` | pagination | Token holder balances |
| GET | `/api/tokens/{address}/transfers` | pagination | Token transfer history |
| GET | `/api/tokens/{address}/chart` | `window` | Token chart data |

Chart `window` values are `1h`, `6h`, `24h`, `7d`, `1m`, `6m`, and `1y`.

## NFTs

| Method | Path | Query | Description |
| --- | --- | --- | --- |
| GET | `/api/nfts/collections` | pagination | List NFT collections |
| GET | `/api/nfts/collections/{address}` | none | Collection detail |
| GET | `/api/nfts/collections/{address}/tokens` | pagination | Tokens in a collection |
| GET | `/api/nfts/collections/{address}/transfers` | pagination | Collection transfer history |
| GET | `/api/nfts/collections/{address}/tokens/{token_id}` | none | NFT token detail |
| GET | `/api/nfts/collections/{address}/tokens/{token_id}/transfers` | pagination | Token transfer history |

NFT token responses include metadata state fields such as `metadata_status`, `metadata_retry_count`, `next_retry_at`, `last_metadata_error`, `last_metadata_attempted_at`, and `metadata_updated_at`.

## Event Logs

| Method | Path | Query | Description |
| --- | --- | --- | --- |
| GET | `/api/transactions/{hash}/logs` | pagination | Logs in a transaction |
| GET | `/api/transactions/{hash}/logs/decoded` | pagination | Decoded logs in a transaction |
| GET | `/api/addresses/{address}/logs` | `topic0`, pagination | Logs emitted by address |

## Contracts and Proxies

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/contracts/{address}` | Verified contract metadata, source, ABI, and verification status |
| POST | `/api/contracts/{address}/verify` | Verify source for a contract address |
| GET | `/api/contracts/{address}/proxy` | Proxy detection result for an address |
| GET | `/api/contracts/{address}/combined-abi` | Merged proxy and implementation ABI when available |
| GET | `/api/proxies` | List detected proxy contracts |

`POST /api/contracts/{address}/verify` accepts JSON for single-file Solidity source or standard JSON compiler input. The route allows up to 50 MiB request bodies and is not subject to the 10 second timeout.

Example body:

```json
{
  "source_code": "contract C {}",
  "standard_json_input": null,
  "contract_name": "C",
  "compiler_version": "v0.8.19+commit.7dd6d404",
  "optimization_enabled": true,
  "optimization_runs": 200,
  "constructor_args": "0x",
  "evm_version": "paris",
  "license_type": "MIT"
}
```

## Search

| Method | Path | Query | Description |
| --- | --- | --- | --- |
| GET | `/api/search` | `q` required | Universal search |

Search result types:

- `block`
- `transaction`
- `address`
- `nft_collection`
- `nft`
- `erc20_token`

Hex queries search addresses, transactions, and block hashes. Numeric queries search block numbers. Text queries of at least three characters search NFT collections, NFT token names, and ERC-20 names/symbols with wildcard escaping.

## Stats

| Method | Path | Query | Description |
| --- | --- | --- | --- |
| GET | `/api/stats/blocks-chart` | `window` | Bucketed transaction count and average gas used |
| GET | `/api/stats/daily-txs` | none | Daily transaction counts for the last 14 indexed days |
| GET | `/api/stats/gas-price` | `window` | Bucketed average gas price with base-fee fallback |

Stats windows support `1h`, `6h`, `24h`, `7d`, `1m`, `6m`, and `1y`.

## Faucet

Faucet routes are registered only when `FAUCET_ENABLED=true` and required faucet settings are valid.

| Method | Path | Description |
| --- | --- | --- |
| GET | `/api/faucet/info` | Faucet amount, backend balance, and cooldown |
| POST | `/api/faucet` | Request funds for an address |

`POST /api/faucet` body:

```json
{
  "address": "0x0000000000000000000000000000000000000001"
}
```

Success:

```json
{
  "tx_hash": "0x..."
}
```

Cooldown failures return `429 Too Many Requests` with retry metadata.

## Etherscan-Compatible API

The exact path `/api` supports Etherscan-style query parameters and response shape:

```json
{
  "status": "1",
  "message": "OK",
  "result": {}
}
```

Supported modules and actions:

```text
GET /api?module=account&action=balance&address=0x...
GET /api?module=account&action=balancemulti&address=0x...,0x...
GET /api?module=account&action=txlist&address=0x...
GET /api?module=account&action=txlistinternal&address=0x...
GET /api?module=account&action=tokentx&address=0x...
GET /api?module=account&action=tokenbalance&address=0x...&contractaddress=0x...

GET /api?module=contract&action=getabi&address=0x...
GET /api?module=contract&action=getsourcecode&address=0x...

GET /api?module=transaction&action=gettxreceiptstatus&txhash=0x...

GET /api?module=block&action=getblockreward&blockno=123

GET /api?module=proxy&action=eth_blockNumber
GET /api?module=proxy&action=eth_getBlockByNumber&blockno=0x1
GET /api?module=proxy&action=eth_getTransactionByHash&txhash=0x...
```

Unsupported modules or actions return an Etherscan-style error payload.
