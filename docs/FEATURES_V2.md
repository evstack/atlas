# Atlas v2 Features

Post-MVP feature additions based on Blockscout analysis. Implement after core explorer is stable.

---

## Priority 1: Essential

### 1.1 ERC-20 Token Support

ERC-721-only is insufficient for a general-purpose explorer.

**Data Model**

```
ERC20Contract:
- address: bytes20 (PK)
- name: string
- symbol: string
- decimals: u8
- total_supply: u256 (optional, not all tokens have this)
- first_seen_block: u64

ERC20Transfer:
- id: serial (PK)
- tx_hash: bytes32 (FK)
- log_index: u32
- contract_address: bytes20 (FK)
- from_address: address
- to_address: address
- value: u256
- block_number: u64
- timestamp: u64

ERC20Balance:
- address: bytes20 (PK)
- contract_address: bytes20 (PK)
- balance: u256
- last_updated_block: u64
```

**API Endpoints**

- `GET /api/tokens` - List ERC-20 tokens
- `GET /api/tokens/:address` - Token detail (name, symbol, decimals, holders count)
- `GET /api/tokens/:address/holders` - Paginated holder list
- `GET /api/tokens/:address/transfers` - Token transfer history
- `GET /api/addresses/:address/tokens` - ERC-20 balances for address

**Indexer Changes**

- Detect ERC-20 via Transfer(address,address,uint256) events (same signature as ERC-721, differentiate by topic count)
- Call `name()`, `symbol()`, `decimals()` on detection
- Track balances incrementally from transfer events

---

### 1.2 Event Log Decoding

Show all emitted events, not just Transfer.

**Data Model**

```
EventLog:
- id: serial (PK)
- tx_hash: bytes32 (FK)
- log_index: u32
- address: bytes20
- topic0: bytes32 (event signature)
- topic1: bytes32 (nullable)
- topic2: bytes32 (nullable)
- topic3: bytes32 (nullable)
- data: bytes
- block_number: u64
- decoded: jsonb (nullable, populated if ABI known)
```

**API Endpoints**

- `GET /api/transactions/:hash/logs` - All logs for transaction
- `GET /api/addresses/:address/logs` - Logs emitted by contract (paginated)
- `GET /api/logs?topic0=:sig` - Filter logs by event signature

**Decoding Strategy**

1. Maintain a table of known event signatures (4bytes.directory or curated list)
2. For verified contracts, use stored ABI
3. Store decoded JSON when possible, raw otherwise

---

### 1.3 Etherscan-Compatible API

Required for tooling ecosystem compatibility.

**Endpoints to Implement**

```
# Account
/api?module=account&action=balance&address=:addr
/api?module=account&action=balancemulti&address=:addr1,:addr2
/api?module=account&action=txlist&address=:addr
/api?module=account&action=txlistinternal&address=:addr
/api?module=account&action=tokentx&address=:addr
/api?module=account&action=tokenbalance&address=:addr&contractaddress=:token

# Contract
/api?module=contract&action=getabi&address=:addr
/api?module=contract&action=getsourcecode&address=:addr
/api?module=contract&action=verifysourcecode (POST)

# Transaction
/api?module=transaction&action=gettxreceiptstatus&txhash=:hash

# Block
/api?module=block&action=getblockreward&blockno=:num

# Proxy (pass-through to RPC)
/api?module=proxy&action=eth_blockNumber
/api?module=proxy&action=eth_getBlockByNumber
/api?module=proxy&action=eth_getTransactionByHash
```

**Implementation Notes**

- Response format must match Etherscan exactly (status, message, result fields)
- Hardhat and Foundry verify plugins depend on specific response shapes
- Consider rate limiting per API key (optional)

---

### 1.4 Address Labels

Zero-complexity UX improvement.

**Implementation**

Config file (`labels.json` or database table):

```json
{
  "0x1234...": {
    "name": "Bridge Contract",
    "tags": ["infrastructure", "bridge"]
  },
  "0x5678...": {
    "name": "Governance",
    "tags": ["governance"]
  }
}
```

**Features**

- Display label instead of/alongside raw address
- Filter addresses by tag
- Admin endpoint to add/update labels (or file reload)
- No user accounts needed - curated list only

---

## Priority 2: High Value

### 2.1 Internal Transactions (Traces)

Required for accurate value tracking in contract interactions.

**Prerequisites**

- L2 node must support `debug_traceTransaction` or `trace_transaction`
- Significantly increases indexing time and storage

**Data Model**

```
InternalTransaction:
- id: serial (PK)
- tx_hash: bytes32 (FK)
- trace_address: int[] (e.g., [0, 1, 2] for nested calls)
- call_type: enum (call, delegatecall, staticcall, create, create2, selfdestruct)
- from_address: address
- to_address: address
- value: u256
- gas: u64
- gas_used: u64
- input: bytes
- output: bytes
- error: string (nullable)
- block_number: u64
```

**API Endpoints**

- `GET /api/transactions/:hash/internal` - Internal txs for transaction
- `GET /api/addresses/:address/internal` - Internal txs involving address

**Indexer Changes**

- Add trace fetching step after receipt processing
- Consider making this optional (config flag) due to performance impact

---

### 2.2 Read/Write Contract Interaction

Direct contract interaction from explorer UI.

**Read Functions**

- Parse ABI from verified contracts
- Generate UI form for view/pure functions
- Execute via `eth_call` and display results
- No wallet connection needed

**Write Functions**

- Generate UI form for state-changing functions
- Connect wallet (WalletConnect, injected provider)
- Estimate gas, submit transaction
- Show pending status, link to tx page on confirmation

**API Support**

- `POST /api/contracts/:address/call` - Execute read function
  ```json
  {
    "function": "balanceOf",
    "args": ["0x1234..."]
  }
  ```

---

### 2.3 Proxy Contract Detection

Critical if L2 uses upgradeable contracts.

**Patterns to Detect**

| Pattern | Storage Slot |
|---------|--------------|
| EIP-1967 | `0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc` |
| EIP-1822 (UUPS) | `0xc5f16f0fcc639fa48a6947836d9850f504798523bf8c9a3a87d5876cf622bcf7` |
| OpenZeppelin Transparent | Admin slot at `0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103` |

**Implementation**

1. On contract detection, check known proxy storage slots
2. If implementation address found, link contracts
3. When viewing proxy, show implementation source/ABI
4. Handle nested proxies (proxy -> proxy -> impl)

**UI Changes**

- Badge indicating "Proxy Contract"
- Link to implementation
- Show combined ABI (proxy + implementation) for interaction

---

## Priority 3: Nice to Have

### 3.1 Basic Chain Analytics

Minimal analytics without a full dashboard system.

**Metrics to Track**

```
DailyStats:
- date: date (PK)
- block_count: u32
- tx_count: u32
- unique_addresses: u32 (senders + receivers)
- gas_used: u256
- avg_gas_price: u256
```

**API Endpoints**

- `GET /api/stats/daily?from=:date&to=:date` - Daily stats
- `GET /api/stats/summary` - Current chain state (latest block, total txs, etc.)

**Implementation**

- Aggregate during indexing or via scheduled job
- Simple line charts on homepage

---

### 3.2 State Diffs

Show storage changes per transaction.

**Data Model**

```
StateDiff:
- id: serial (PK)
- tx_hash: bytes32 (FK)
- address: bytes20
- slot: bytes32
- previous_value: bytes32
- new_value: bytes32
```

**Requirements**

- Requires `debug_traceTransaction` with `prestateTracer` or `stateDiffTracer`
- High storage cost - consider storing only for recent blocks or on-demand

---

## Implementation Order

```
Phase 1 (Post-MVP Stabilization)
├── 1.4 Address Labels (trivial, immediate UX win)
├── 1.1 ERC-20 Token Support
└── 1.2 Event Log Decoding

Phase 2
├── 1.3 Etherscan-Compatible API
├── 2.3 Proxy Contract Detection
└── 2.2 Read/Write Contract Interaction

Phase 3 (If Needed)
├── 2.1 Internal Transactions
├── 3.1 Basic Chain Analytics
└── 3.2 State Diffs
```

---

## Dependencies & Risks

| Feature | Dependency | Risk |
|---------|------------|------|
| Internal Transactions | `debug_*` RPC methods | Node may not support, 10x+ indexing time |
| State Diffs | `debug_traceTransaction` | Same as above |
| Write Contract | Wallet integration | Frontend complexity, security surface |
| ERC-20 Balances | Accurate transfer indexing | Reorg handling critical for balance accuracy |

---

## Non-Goals (Unchanged from v1)

- Multi-chain support
- User accounts / authentication
- Gas price oracles
- Full analytics dashboards
- ERC-1155 support (reconsider if needed)
