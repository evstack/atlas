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

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;

    // --- touch_addr tests ---

    #[test]
    fn touch_addr_first_insertion() {
        let mut batch = BlockBatch::new();
        batch.touch_addr("0xabc".to_string(), 100, false, 1);

        let state = batch.addr_map.get("0xabc").unwrap();
        assert_eq!(state.first_seen_block, 100);
        assert!(!state.is_contract);
        assert_eq!(state.tx_count_delta, 1);
    }

    #[test]
    fn touch_addr_keeps_minimum_first_seen_block() {
        let mut batch = BlockBatch::new();
        batch.touch_addr("0xabc".to_string(), 200, false, 0);
        batch.touch_addr("0xabc".to_string(), 100, false, 0);

        assert_eq!(batch.addr_map["0xabc"].first_seen_block, 100);
    }

    #[test]
    fn touch_addr_first_seen_block_does_not_increase() {
        let mut batch = BlockBatch::new();
        batch.touch_addr("0xabc".to_string(), 100, false, 0);
        batch.touch_addr("0xabc".to_string(), 200, false, 0);

        assert_eq!(batch.addr_map["0xabc"].first_seen_block, 100);
    }

    #[test]
    fn touch_addr_is_contract_latches_true() {
        let mut batch = BlockBatch::new();
        batch.touch_addr("0xabc".to_string(), 100, false, 0);
        batch.touch_addr("0xabc".to_string(), 101, true, 0);

        assert!(batch.addr_map["0xabc"].is_contract);
    }

    #[test]
    fn touch_addr_is_contract_once_true_stays_true() {
        let mut batch = BlockBatch::new();
        batch.touch_addr("0xabc".to_string(), 100, true, 0);
        batch.touch_addr("0xabc".to_string(), 101, false, 0);

        assert!(batch.addr_map["0xabc"].is_contract);
    }

    #[test]
    fn touch_addr_accumulates_tx_count_delta() {
        let mut batch = BlockBatch::new();
        batch.touch_addr("0xabc".to_string(), 100, false, 1);
        batch.touch_addr("0xabc".to_string(), 101, false, 2);
        batch.touch_addr("0xabc".to_string(), 102, false, 3);

        assert_eq!(batch.addr_map["0xabc"].tx_count_delta, 6);
    }

    #[test]
    fn touch_addr_deduplicates_different_addresses_separately() {
        let mut batch = BlockBatch::new();
        batch.touch_addr("0xaaa".to_string(), 100, false, 1);
        batch.touch_addr("0xbbb".to_string(), 200, true, 2);

        assert_eq!(batch.addr_map.len(), 2);
        assert_eq!(batch.addr_map["0xaaa"].tx_count_delta, 1);
        assert_eq!(batch.addr_map["0xbbb"].tx_count_delta, 2);
        assert!(!batch.addr_map["0xaaa"].is_contract);
        assert!(batch.addr_map["0xbbb"].is_contract);
    }

    // --- apply_balance_delta tests ---

    #[test]
    fn apply_balance_delta_first_insertion() {
        let mut batch = BlockBatch::new();
        batch.apply_balance_delta(
            "0xaddr".to_string(),
            "0xtoken".to_string(),
            BigDecimal::from(100),
            50,
        );

        let entry = batch
            .balance_map
            .get(&("0xaddr".to_string(), "0xtoken".to_string()))
            .unwrap();
        assert_eq!(entry.delta, BigDecimal::from(100));
        assert_eq!(entry.last_block, 50);
    }

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
    fn apply_balance_delta_accumulates_negative() {
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
            BigDecimal::from(-30),
            51,
        );

        let entry = batch
            .balance_map
            .get(&("0xaddr".to_string(), "0xtoken".to_string()))
            .unwrap();
        assert_eq!(entry.delta, BigDecimal::from(70));
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
    fn apply_balance_delta_separate_contracts_are_independent() {
        let mut batch = BlockBatch::new();
        batch.apply_balance_delta(
            "0xaddr".to_string(),
            "0xtoken1".to_string(),
            BigDecimal::from(100),
            50,
        );
        batch.apply_balance_delta(
            "0xaddr".to_string(),
            "0xtoken2".to_string(),
            BigDecimal::from(200),
            50,
        );

        assert_eq!(batch.balance_map.len(), 2);
        assert_eq!(
            batch
                .balance_map
                .get(&("0xaddr".to_string(), "0xtoken1".to_string()))
                .unwrap()
                .delta,
            BigDecimal::from(100)
        );
        assert_eq!(
            batch
                .balance_map
                .get(&("0xaddr".to_string(), "0xtoken2".to_string()))
                .unwrap()
                .delta,
            BigDecimal::from(200)
        );
    }
}
