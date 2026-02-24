use bigdecimal::BigDecimal;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Batch accumulator - collects data from multiple blocks before writing to DB
// ---------------------------------------------------------------------------

pub(crate) struct AddrState {
    pub(crate) first_seen_block: i64,
    pub(crate) is_contract: bool,
    pub(crate) tx_count_delta: i64,
}

pub(crate) struct NftTokenState {
    pub(crate) owner: String,
    pub(crate) last_transfer_block: i64,
}

pub(crate) struct BalanceDelta {
    pub(crate) delta: BigDecimal,
    pub(crate) last_block: i64,
}

/// Holds all data collected across a batch of blocks, ready for bulk insert.
/// Fields are columnar (parallel Vecs) so they can be passed directly to
/// PostgreSQL UNNEST without any further transformation.
#[derive(Default)]
pub(crate) struct BlockBatch {
    // blocks
    pub(crate) b_numbers: Vec<i64>,
    pub(crate) b_hashes: Vec<String>,
    pub(crate) b_parent_hashes: Vec<String>,
    pub(crate) b_timestamps: Vec<i64>,
    pub(crate) b_gas_used: Vec<i64>,
    pub(crate) b_gas_limits: Vec<i64>,
    pub(crate) b_tx_counts: Vec<i32>,

    // transactions (receipt data merged in at collection time)
    pub(crate) t_hashes: Vec<String>,
    pub(crate) t_block_numbers: Vec<i64>,
    pub(crate) t_block_indices: Vec<i32>,
    pub(crate) t_froms: Vec<String>,
    pub(crate) t_tos: Vec<Option<String>>,
    pub(crate) t_values: Vec<String>, // BigDecimal as string → cast to numeric in SQL
    pub(crate) t_gas_prices: Vec<String>, // BigDecimal as string → cast to numeric in SQL
    pub(crate) t_gas_used: Vec<i64>,
    pub(crate) t_input_data: Vec<Vec<u8>>,
    pub(crate) t_statuses: Vec<bool>,
    pub(crate) t_timestamps: Vec<i64>,
    pub(crate) t_contracts_created: Vec<Option<String>>,

    // tx_hash_lookup
    pub(crate) tl_hashes: Vec<String>,
    pub(crate) tl_block_numbers: Vec<i64>,

    // addresses — deduplicated by address in Rust
    pub(crate) addr_map: HashMap<String, AddrState>,

    // event_logs
    pub(crate) el_tx_hashes: Vec<String>,
    pub(crate) el_log_indices: Vec<i32>,
    pub(crate) el_addresses: Vec<String>,
    pub(crate) el_topic0s: Vec<String>,
    pub(crate) el_topic1s: Vec<Option<String>>,
    pub(crate) el_topic2s: Vec<Option<String>>,
    pub(crate) el_topic3s: Vec<Option<String>>,
    pub(crate) el_datas: Vec<Vec<u8>>,
    pub(crate) el_block_numbers: Vec<i64>,

    // nft_contracts — deduplicated
    pub(crate) nft_contract_addrs: Vec<String>,
    pub(crate) nft_contract_first_seen: Vec<i64>,

    // nft_transfers
    pub(crate) nt_tx_hashes: Vec<String>,
    pub(crate) nt_log_indices: Vec<i32>,
    pub(crate) nt_contracts: Vec<String>,
    pub(crate) nt_token_ids: Vec<String>, // BigDecimal as string
    pub(crate) nt_froms: Vec<String>,
    pub(crate) nt_tos: Vec<String>,
    pub(crate) nt_block_numbers: Vec<i64>,
    pub(crate) nt_timestamps: Vec<i64>,

    // nft_tokens — deduplicated: only the latest transfer per token is kept
    pub(crate) nft_token_map: HashMap<(String, String), NftTokenState>,

    // erc20_contracts — new contracts only (no inline metadata fetch)
    pub(crate) ec_addresses: Vec<String>,
    pub(crate) ec_first_seen_blocks: Vec<i64>,

    // erc20_transfers
    pub(crate) et_tx_hashes: Vec<String>,
    pub(crate) et_log_indices: Vec<i32>,
    pub(crate) et_contracts: Vec<String>,
    pub(crate) et_froms: Vec<String>,
    pub(crate) et_tos: Vec<String>,
    pub(crate) et_values: Vec<String>, // BigDecimal as string
    pub(crate) et_block_numbers: Vec<i64>,
    pub(crate) et_timestamps: Vec<i64>,

    // erc20_balances — aggregated deltas per (address, contract)
    pub(crate) balance_map: HashMap<(String, String), BalanceDelta>,

    // Contracts newly discovered in this batch.
    // These are NOT merged into the persistent known_* sets until after a
    // successful write, so a failed write doesn't leave the in-memory sets
    // ahead of the database.
    pub(crate) new_erc20: HashSet<String>,
    pub(crate) new_nft: HashSet<String>,

    pub(crate) last_block: u64,
}

impl BlockBatch {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Upsert an address into the in-memory deduplication map.
    /// tx_count_delta is added to whatever was already accumulated for this address.
    pub(crate) fn touch_addr(
        &mut self,
        address: String,
        block_num: i64,
        is_contract: bool,
        tx_count_delta: i64,
    ) {
        let entry = self.addr_map.entry(address).or_insert(AddrState {
            first_seen_block: block_num,
            is_contract: false,
            tx_count_delta: 0,
        });
        entry.first_seen_block = entry.first_seen_block.min(block_num);
        entry.is_contract |= is_contract;
        entry.tx_count_delta += tx_count_delta;
    }

    /// Add a balance delta for (address, contract).
    /// Multiple transfers in the same batch are aggregated into one row.
    pub(crate) fn apply_balance_delta(
        &mut self,
        address: String,
        contract: String,
        delta: BigDecimal,
        block: i64,
    ) {
        let entry = self
            .balance_map
            .entry((address, contract))
            .or_insert(BalanceDelta {
                delta: BigDecimal::from(0),
                last_block: block,
            });
        entry.delta += delta;
        entry.last_block = entry.last_block.max(block);
    }
}
