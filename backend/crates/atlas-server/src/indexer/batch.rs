use bigdecimal::BigDecimal;
use std::collections::{HashMap, HashSet};

use atlas_common::Block;
use chrono::{DateTime, Utc};

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

    // erc20 total supply deltas — aggregated per contract from mint/burn events
    pub(crate) supply_map: HashMap<String, BigDecimal>,

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

    /// Add a total supply delta for a contract.
    /// Only mint and burn transfers should touch this accumulator.
    pub(crate) fn apply_supply_delta(&mut self, contract: String, delta: BigDecimal) {
        let entry = self
            .supply_map
            .entry(contract)
            .or_insert(BigDecimal::from(0));
        *entry += delta;
    }

    pub(crate) fn materialize_blocks(&self, indexed_at: DateTime<Utc>) -> Vec<Block> {
        debug_assert_eq!(self.b_numbers.len(), self.b_hashes.len());
        debug_assert_eq!(self.b_numbers.len(), self.b_parent_hashes.len());
        debug_assert_eq!(self.b_numbers.len(), self.b_timestamps.len());
        debug_assert_eq!(self.b_numbers.len(), self.b_gas_used.len());
        debug_assert_eq!(self.b_numbers.len(), self.b_gas_limits.len());
        debug_assert_eq!(self.b_numbers.len(), self.b_tx_counts.len());

        (0..self.b_numbers.len())
            .map(|i| Block {
                number: self.b_numbers[i],
                hash: self.b_hashes[i].clone(),
                parent_hash: self.b_parent_hashes[i].clone(),
                timestamp: self.b_timestamps[i],
                gas_used: self.b_gas_used[i],
                gas_limit: self.b_gas_limits[i],
                transaction_count: self.b_tx_counts[i],
                indexed_at,
            })
            .collect()
    }

    pub(crate) fn last_block_timestamp(&self) -> Option<i64> {
        self.b_timestamps.last().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use chrono::{TimeZone, Utc};

    // --- touch_addr tests ---

    #[test]
    fn touch_addr_keeps_minimum_first_seen_block() {
        let mut batch = BlockBatch::new();
        batch.touch_addr("0xabc".to_string(), 200, false, 0);
        batch.touch_addr("0xabc".to_string(), 100, false, 0);

        assert_eq!(batch.addr_map["0xabc"].first_seen_block, 100);
    }

    #[test]
    fn touch_addr_is_contract_latches_true() {
        let mut batch = BlockBatch::new();
        batch.touch_addr("0xabc".to_string(), 100, false, 0);
        batch.touch_addr("0xabc".to_string(), 101, true, 0);

        assert!(batch.addr_map["0xabc"].is_contract);
    }

    // --- apply_balance_delta tests ---

    #[test]
    fn apply_balance_delta_accumulates_positive() {
        let mut batch = BlockBatch::new();
        batch.apply_balance_delta(
            "0xaddr".to_string(),
            "0xtoken".to_string(),
            BigDecimal::from(100),
            50,
        );
        batch.apply_balance_delta(
            "0xaddr".to_string(),
            "0xtoken".to_string(),
            BigDecimal::from(50),
            60,
        );

        let entry = batch
            .balance_map
            .get(&("0xaddr".to_string(), "0xtoken".to_string()))
            .unwrap();
        assert_eq!(entry.delta, BigDecimal::from(150));
        assert_eq!(entry.last_block, 60);
    }

    #[test]
    fn apply_balance_delta_tracks_max_block() {
        let mut batch = BlockBatch::new();
        batch.apply_balance_delta(
            "0xaddr".to_string(),
            "0xtoken".to_string(),
            BigDecimal::from(1),
            100,
        );
        // Earlier block — last_block should stay at 100
        batch.apply_balance_delta(
            "0xaddr".to_string(),
            "0xtoken".to_string(),
            BigDecimal::from(1),
            50,
        );

        let entry = batch
            .balance_map
            .get(&("0xaddr".to_string(), "0xtoken".to_string()))
            .unwrap();
        assert_eq!(entry.last_block, 100);
    }

    #[test]
    fn apply_supply_delta_accumulates_by_contract() {
        let mut batch = BlockBatch::new();
        let contract = "0xtoken".to_string();

        batch.apply_supply_delta(contract.clone(), BigDecimal::from(100));
        batch.apply_supply_delta(contract.clone(), BigDecimal::from(-25));

        assert_eq!(batch.supply_map[&contract], BigDecimal::from(75));
    }

    #[test]
    fn materialize_blocks_preserves_parallel_block_fields() {
        let mut batch = BlockBatch::new();
        batch.b_numbers.push(42);
        batch.b_hashes.push("0xabc".to_string());
        batch.b_parent_hashes.push("0xdef".to_string());
        batch.b_timestamps.push(1_700_000_042);
        batch.b_gas_used.push(21_000);
        batch.b_gas_limits.push(30_000_000);
        batch.b_tx_counts.push(3);

        let indexed_at = Utc.timestamp_opt(1_700_000_100, 0).unwrap();
        let blocks = batch.materialize_blocks(indexed_at);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].number, 42);
        assert_eq!(blocks[0].hash, "0xabc");
        assert_eq!(blocks[0].parent_hash, "0xdef");
        assert_eq!(blocks[0].timestamp, 1_700_000_042);
        assert_eq!(blocks[0].gas_used, 21_000);
        assert_eq!(blocks[0].gas_limit, 30_000_000);
        assert_eq!(blocks[0].transaction_count, 3);
        assert_eq!(blocks[0].indexed_at, indexed_at);
    }

    #[test]
    fn last_block_timestamp_returns_latest_collected_timestamp() {
        let mut batch = BlockBatch::new();
        batch.b_timestamps.push(1_700_000_001);
        batch.b_timestamps.push(1_700_000_042);

        assert_eq!(batch.last_block_timestamp(), Some(1_700_000_042));
    }
}
