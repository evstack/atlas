use alloy::network::Ethereum;
use alloy::primitives::U256;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::types::{Block, TransactionReceipt};
use alloy::transports::http::{Client, Http};
use anyhow::Result;
use bigdecimal::BigDecimal;
use governor::{Quota, RateLimiter};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::num::NonZeroU32;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::Config;

/// Retry delays for RPC calls (in seconds)
const RPC_RETRY_DELAYS: &[u64] = &[2, 5, 10, 20, 30];
const RPC_MAX_RETRIES: usize = 10;

/// Partition size: 10 million blocks per partition
const PARTITION_SIZE: u64 = 10_000_000;

/// ERC-20/721 Transfer event signature: Transfer(address,address,uint256)
const TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

/// Work item for a worker - a range of blocks to fetch
#[derive(Debug, Clone)]
struct WorkItem {
    start_block: u64,
    count: usize,
}

type HttpProvider = RootProvider<Http<Client>, Ethereum>;
type SharedRateLimiter = Arc<RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>;

/// Result of fetching a block from RPC
enum FetchResult {
    Success(FetchedBlock),
    Error { block_num: u64, error: String },
}

/// Data fetched from RPC for a single block
struct FetchedBlock {
    number: u64,
    block: Block,
    receipts: Vec<TransactionReceipt>,
}

// ---------------------------------------------------------------------------
// Batch accumulator - collects data from multiple blocks before writing to DB
// ---------------------------------------------------------------------------

struct AddrState {
    first_seen_block: i64,
    is_contract: bool,
    tx_count_delta: i64,
}

struct NftTokenState {
    owner: String,
    last_transfer_block: i64,
}

struct BalanceDelta {
    delta: BigDecimal,
    last_block: i64,
}

/// Holds all data collected across a batch of blocks, ready for bulk insert.
/// Fields are columnar (parallel Vecs) so they can be passed directly to
/// PostgreSQL UNNEST without any further transformation.
struct BlockBatch {
    // blocks
    b_numbers: Vec<i64>,
    b_hashes: Vec<String>,
    b_parent_hashes: Vec<String>,
    b_timestamps: Vec<i64>,
    b_gas_used: Vec<i64>,
    b_gas_limits: Vec<i64>,
    b_tx_counts: Vec<i32>,

    // transactions (receipt data merged in at collection time)
    t_hashes: Vec<String>,
    t_block_numbers: Vec<i64>,
    t_block_indices: Vec<i32>,
    t_froms: Vec<String>,
    t_tos: Vec<Option<String>>,
    t_values: Vec<String>,         // BigDecimal as string → cast to numeric in SQL
    t_gas_prices: Vec<String>,     // BigDecimal as string → cast to numeric in SQL
    t_gas_used: Vec<i64>,
    t_input_data: Vec<Vec<u8>>,
    t_statuses: Vec<bool>,
    t_timestamps: Vec<i64>,
    t_contracts_created: Vec<Option<String>>,

    // tx_hash_lookup
    tl_hashes: Vec<String>,
    tl_block_numbers: Vec<i64>,

    // addresses — deduplicated by address in Rust
    addr_map: HashMap<String, AddrState>,

    // event_logs
    el_tx_hashes: Vec<String>,
    el_log_indices: Vec<i32>,
    el_addresses: Vec<String>,
    el_topic0s: Vec<String>,
    el_topic1s: Vec<Option<String>>,
    el_topic2s: Vec<Option<String>>,
    el_topic3s: Vec<Option<String>>,
    el_datas: Vec<Vec<u8>>,
    el_block_numbers: Vec<i64>,

    // nft_contracts — deduplicated
    nft_contract_addrs: Vec<String>,
    nft_contract_first_seen: Vec<i64>,

    // nft_transfers
    nt_tx_hashes: Vec<String>,
    nt_log_indices: Vec<i32>,
    nt_contracts: Vec<String>,
    nt_token_ids: Vec<String>,  // BigDecimal as string
    nt_froms: Vec<String>,
    nt_tos: Vec<String>,
    nt_block_numbers: Vec<i64>,
    nt_timestamps: Vec<i64>,

    // nft_tokens — deduplicated: only the latest transfer per token is kept
    nft_token_map: HashMap<(String, String), NftTokenState>,

    // erc20_contracts — new contracts only (no inline metadata fetch)
    ec_addresses: Vec<String>,
    ec_first_seen_blocks: Vec<i64>,

    // erc20_transfers
    et_tx_hashes: Vec<String>,
    et_log_indices: Vec<i32>,
    et_contracts: Vec<String>,
    et_froms: Vec<String>,
    et_tos: Vec<String>,
    et_values: Vec<String>,   // BigDecimal as string
    et_block_numbers: Vec<i64>,
    et_timestamps: Vec<i64>,

    // erc20_balances — aggregated deltas per (address, contract)
    balance_map: HashMap<(String, String), BalanceDelta>,

    // Contracts newly discovered in this batch.
    // These are NOT merged into the persistent known_* sets until after a
    // successful write, so a failed write doesn't leave the in-memory sets
    // ahead of the database.
    new_erc20: HashSet<String>,
    new_nft: HashSet<String>,

    last_block: u64,
}

impl BlockBatch {
    fn new() -> Self {
        Self {
            b_numbers: vec![],
            b_hashes: vec![],
            b_parent_hashes: vec![],
            b_timestamps: vec![],
            b_gas_used: vec![],
            b_gas_limits: vec![],
            b_tx_counts: vec![],
            t_hashes: vec![],
            t_block_numbers: vec![],
            t_block_indices: vec![],
            t_froms: vec![],
            t_tos: vec![],
            t_values: vec![],
            t_gas_prices: vec![],
            t_gas_used: vec![],
            t_input_data: vec![],
            t_statuses: vec![],
            t_timestamps: vec![],
            t_contracts_created: vec![],
            tl_hashes: vec![],
            tl_block_numbers: vec![],
            addr_map: HashMap::new(),
            el_tx_hashes: vec![],
            el_log_indices: vec![],
            el_addresses: vec![],
            el_topic0s: vec![],
            el_topic1s: vec![],
            el_topic2s: vec![],
            el_topic3s: vec![],
            el_datas: vec![],
            el_block_numbers: vec![],
            nft_contract_addrs: vec![],
            nft_contract_first_seen: vec![],
            nt_tx_hashes: vec![],
            nt_log_indices: vec![],
            nt_contracts: vec![],
            nt_token_ids: vec![],
            nt_froms: vec![],
            nt_tos: vec![],
            nt_block_numbers: vec![],
            nt_timestamps: vec![],
            nft_token_map: HashMap::new(),
            ec_addresses: vec![],
            ec_first_seen_blocks: vec![],
            et_tx_hashes: vec![],
            et_log_indices: vec![],
            et_contracts: vec![],
            et_froms: vec![],
            et_tos: vec![],
            et_values: vec![],
            et_block_numbers: vec![],
            et_timestamps: vec![],
            balance_map: HashMap::new(),
            new_erc20: HashSet::new(),
            new_nft: HashSet::new(),
            last_block: 0,
        }
    }

    /// Upsert an address into the in-memory deduplication map.
    /// tx_count_delta is added to whatever was already accumulated for this address.
    fn touch_addr(&mut self, address: String, block_num: i64, is_contract: bool, tx_count_delta: i64) {
        let entry = self.addr_map.entry(address).or_insert(AddrState {
            first_seen_block: block_num,
            is_contract: false,
            tx_count_delta: 0,
        });
        if block_num < entry.first_seen_block {
            entry.first_seen_block = block_num;
        }
        entry.is_contract = entry.is_contract || is_contract;
        entry.tx_count_delta += tx_count_delta;
    }

    /// Add a balance delta for (address, contract).
    /// Multiple transfers in the same batch are aggregated into one row.
    fn apply_balance_delta(&mut self, address: String, contract: String, delta: BigDecimal, block: i64) {
        let entry = self.balance_map.entry((address, contract)).or_insert(BalanceDelta {
            delta: BigDecimal::from(0),
            last_block: block,
        });
        entry.delta = entry.delta.clone() + delta;
        if block > entry.last_block {
            entry.last_block = block;
        }
    }
}

// ---------------------------------------------------------------------------
// Indexer
// ---------------------------------------------------------------------------

pub struct Indexer {
    pool: PgPool,
    config: Config,
    /// Tracks the maximum partition number that has been created
    /// Used to avoid checking pg_class on every batch
    current_max_partition: std::sync::atomic::AtomicU64,
}

impl Indexer {
    pub fn new(pool: PgPool, config: Config) -> Self {
        Self {
            pool,
            config,
            // Will be initialized on first run based on start block
            current_max_partition: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub async fn run(&self) -> Result<()> {
        let provider = Arc::new(ProviderBuilder::new().on_http(self.config.rpc_url.parse()?));

        // Create rate limiter for RPC requests
        let rps = NonZeroU32::new(self.config.rpc_requests_per_second).unwrap_or(NonZeroU32::new(100).unwrap());
        let rate_limiter: SharedRateLimiter = Arc::new(RateLimiter::direct(Quota::per_second(rps)));
        tracing::info!("Rate limiting RPC requests to {} req/sec", rps);

        // Handle reindex flag
        if self.config.reindex {
            tracing::warn!("Reindex flag set - truncating all tables");
            self.truncate_tables().await?;
        }

        // Get starting block
        let start_block = self.get_start_block().await?;
        tracing::info!("Starting indexing from block {}", start_block);

        // Load known contracts into memory to avoid a SELECT per transfer
        let mut known_erc20: HashSet<String> = self.load_known_erc20().await?;
        tracing::info!("Loaded {} known ERC-20 contracts", known_erc20.len());
        let mut known_nft: HashSet<String> = self.load_known_nft().await?;
        tracing::info!("Loaded {} known NFT contracts", known_nft.len());

        let num_workers = self.config.fetch_workers as usize;
        let rpc_batch_size = self.config.rpc_batch_size as usize;
        tracing::info!("Starting {} fetch workers with {} blocks per RPC batch", num_workers, rpc_batch_size);

        // Channels for work distribution and results
        // work_tx: send WorkItems (block ranges) to fetch workers
        // result_tx: workers send fetched blocks back to main loop
        let (work_tx, work_rx) = async_channel::bounded::<WorkItem>(num_workers * 2);
        let (result_tx, mut result_rx) = mpsc::channel::<FetchResult>(num_workers * rpc_batch_size * 2);

        // Create HTTP client for batch requests
        let http_client = reqwest::Client::new();
        let rpc_url = self.config.rpc_url.clone();

        // Spawn long-lived workers
        for worker_id in 0..num_workers {
            let work_rx = work_rx.clone();
            let result_tx = result_tx.clone();
            let limiter = Arc::clone(&rate_limiter);
            let client = http_client.clone();
            let url = rpc_url.clone();

            tokio::spawn(async move {
                tracing::debug!("Worker {} started", worker_id);
                loop {
                    // Wait for work (blocks here until work arrives)
                    match work_rx.recv().await {
                        Ok(work_item) => {
                            // Fetch batch of blocks using JSON-RPC batching
                            let results = Self::fetch_blocks_batch(
                                &client,
                                &url,
                                work_item.start_block,
                                work_item.count,
                                &limiter,
                            ).await;

                            // Send all results back
                            for result in results {
                                if result_tx.send(result).await.is_err() {
                                    return; // Channel closed
                                }
                            }
                        }
                        Err(_) => {
                            // Channel closed, exit worker
                            break;
                        }
                    }
                }
                tracing::debug!("Worker {} shutting down", worker_id);
            });
        }

        // Drop our copy of result_tx so channel closes when all workers are done
        drop(result_tx);

        // Main indexing loop
        let mut current_block = start_block;
        let mut last_log_time = std::time::Instant::now();

        loop {
            // Get chain head with retry
            let head = match self.get_block_number_with_retry(&provider).await {
                Ok(h) => h,
                Err(e) => {
                    // This should only happen after all retries exhausted (very unlikely)
                    // Return error to trigger outer retry which will restart workers
                    return Err(e);
                }
            };
            tracing::debug!("Chain head: {}, current: {}", head, current_block);

            if current_block > head {
                // At head, wait for new blocks
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }

            // Calculate batch end
            let end_block = (current_block + self.config.batch_size - 1).min(head);
            let batch_size = (end_block - current_block + 1) as usize;
            tracing::debug!("Fetching batch: {} to {} ({} blocks)", current_block, end_block, batch_size);

            // Ensure partitions exist for this batch range
            self.ensure_partitions_exist(end_block).await?;

            // Spawn a task to send work (avoids deadlock with bounded channels)
            let work_tx_clone = work_tx.clone();
            let blocks_per_batch = rpc_batch_size;
            let send_task = tokio::spawn(async move {
                let mut block = current_block;
                while block <= end_block {
                    let count = ((end_block - block + 1) as usize).min(blocks_per_batch);
                    let work_item = WorkItem {
                        start_block: block,
                        count,
                    };
                    if work_tx_clone.send(work_item).await.is_err() {
                        break;
                    }
                    block += count as u64;
                }
                tracing::debug!("Sent {} blocks to workers in batches of {}", batch_size, blocks_per_batch);
            });

            // Collect results with reorder buffer, accumulating into a single
            // BlockBatch in order — no DB calls yet.
            let mut buffer: BTreeMap<u64, FetchedBlock> = BTreeMap::new();
            let mut next_to_process = current_block;
            let mut blocks_received = 0;
            let mut failed_blocks: Vec<(u64, String)> = Vec::new();
            let mut batch = BlockBatch::new();

            // Receive all blocks for this batch
            while blocks_received < batch_size {
                match result_rx.recv().await {
                    Some(FetchResult::Success(fetched)) => {
                        buffer.insert(fetched.number, fetched);
                        blocks_received += 1;

                        // Collect consecutive blocks in order (sync, no await)
                        while let Some(data) = buffer.remove(&next_to_process) {
                            Self::collect_block(&mut batch, &known_erc20, &known_nft, data);
                            next_to_process += 1;
                        }
                    }
                    Some(FetchResult::Error { block_num, error }) => {
                        tracing::warn!("Block {} failed to fetch: {}", block_num, error);
                        failed_blocks.push((block_num, error));
                        blocks_received += 1;
                        // Skip this block for now, continue with others
                        if next_to_process == block_num {
                            next_to_process += 1;
                        }
                    }
                    None => {
                        // All workers died unexpectedly
                        return Err(anyhow::anyhow!("All fetch workers terminated"));
                    }
                }
            }

            // Extract newly discovered contracts before consuming the batch.
            // We only merge them into the persistent sets after a successful write —
            // if write_batch fails, the sets stay consistent with the DB.
            let new_erc20 = std::mem::take(&mut batch.new_erc20);
            let new_nft = std::mem::take(&mut batch.new_nft);

            // One DB transaction for the entire batch
            self.write_batch(batch).await?;

            // Write succeeded — now safe to update the persistent in-memory sets
            known_erc20.extend(new_erc20);
            known_nft.extend(new_nft);

            // Wait for send task to complete
            let _ = send_task.await;

            // Retry failed blocks if any
            if !failed_blocks.is_empty() {
                let block_nums: Vec<u64> = failed_blocks.iter().map(|(n, _)| *n).collect();
                tracing::warn!("Retrying {} failed blocks: {:?}", failed_blocks.len(), block_nums);

                // Retry up to 3 times with increasing delay
                for attempt in 1..=3 {
                    if failed_blocks.is_empty() { break; }

                    let delay = Duration::from_secs(attempt * 2); // 2s, 4s, 6s
                    tracing::info!("Retry attempt {} for {} blocks (waiting {:?})",
                        attempt, failed_blocks.len(), delay);
                    tokio::time::sleep(delay).await;

                    let mut still_failed = Vec::new();
                    for (block_num, last_error) in failed_blocks {
                        // Fetch single block
                        let results = Self::fetch_blocks_batch(
                            &http_client,
                            &rpc_url,
                            block_num,
                            1,
                            &rate_limiter,
                        ).await;

                        match results.into_iter().next() {
                            Some(FetchResult::Success(fetched)) => {
                                // Write retried block immediately
                                let mut mini_batch = BlockBatch::new();
                                Self::collect_block(&mut mini_batch, &known_erc20, &known_nft, fetched);
                                let new_erc20 = std::mem::take(&mut mini_batch.new_erc20);
                                let new_nft = std::mem::take(&mut mini_batch.new_nft);
                                self.write_batch(mini_batch).await?;
                                known_erc20.extend(new_erc20);
                                known_nft.extend(new_nft);
                                tracing::info!("Block {} retry succeeded", block_num);
                            }
                            Some(FetchResult::Error { error, .. }) => {
                                still_failed.push((block_num, error));
                            }
                            None => {
                                still_failed.push((block_num, last_error));
                            }
                        }
                    }
                    failed_blocks = still_failed;
                }

                // Store any remaining failures in failed_blocks table
                if !failed_blocks.is_empty() {
                    tracing::error!(
                        "Storing {} blocks in failed_blocks table after 3 retries: {:?}",
                        failed_blocks.len(),
                        failed_blocks.iter().map(|(n, _)| n).collect::<Vec<_>>()
                    );

                    for (block_num, error) in &failed_blocks {
                        sqlx::query(
                            "INSERT INTO failed_blocks (block_number, error_message, retry_count, last_failed_at)
                             VALUES ($1, $2, 3, NOW())
                             ON CONFLICT (block_number) DO UPDATE SET
                                error_message = $2,
                                retry_count = failed_blocks.retry_count + 3,
                                last_failed_at = NOW()"
                        )
                        .bind(*block_num as i64)
                        .bind(error)
                        .execute(&self.pool)
                        .await?;
                    }
                }
            }

            current_block = end_block + 1;

            // Log progress after every batch
            let elapsed = last_log_time.elapsed();
            let blocks_per_sec = batch_size as f64 / elapsed.as_secs_f64();
            let progress = (end_block as f64 / head as f64) * 100.0;

            tracing::info!(
                "Batch complete: {} to {} ({} blocks in {:.2}s = {:.1} blocks/sec) | Progress: {:.2}%",
                end_block - batch_size as u64 + 1, end_block, batch_size, elapsed.as_secs_f64(), blocks_per_sec, progress
            );

            last_log_time = std::time::Instant::now();

            // If we hit the head (batch smaller than configured), sleep to avoid tight loop
            if (batch_size as u64) < self.config.batch_size {
                tracing::debug!("At chain head, sleeping for 1s");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    // -----------------------------------------------------------------------
    // collect_block — pure sync, no DB, no async.
    // Accumulates all block data into the batch for later bulk insert.
    // -----------------------------------------------------------------------

    fn collect_block(batch: &mut BlockBatch, known_erc20: &HashSet<String>, known_nft: &HashSet<String>, fetched: FetchedBlock) {
        use alloy::consensus::Transaction as TxTrait;

        let block = fetched.block;
        let block_num = fetched.number;

        // Build a receipt map keyed by tx hash for O(1) lookup.
        // This lets us merge receipt data (status, gas_used, contract_created)
        // directly into the transaction row, eliminating the UPDATE after INSERT.
        let receipt_map: HashMap<String, &TransactionReceipt> = fetched.receipts
            .iter()
            .map(|r| (format!("{:?}", r.transaction_hash), r))
            .collect();

        // --- Block ---
        let tx_count = block.transactions.len() as i32;
        batch.b_numbers.push(block_num as i64);
        batch.b_hashes.push(format!("{:?}", block.header.hash));
        batch.b_parent_hashes.push(format!("{:?}", block.header.parent_hash));
        batch.b_timestamps.push(block.header.timestamp as i64);
        batch.b_gas_used.push(block.header.gas_used as i64);
        batch.b_gas_limits.push(block.header.gas_limit as i64);
        batch.b_tx_counts.push(tx_count);

        // --- Transactions ---
        if let Some(txs) = block.transactions.as_transactions() {
            for (idx, transaction) in txs.iter().enumerate() {
                let inner = &transaction.inner;
                let tx_hash_str = format!("{:?}", inner.tx_hash());
                let from_str = format!("{:?}", transaction.from);
                let to_opt = inner.to().map(|a| format!("{:?}", a));
                let value_str = inner.value().to_string();
                let gas_price_str = transaction.effective_gas_price
                    .map(|gp| gp.to_string())
                    .unwrap_or_else(|| "0".to_string());
                let input = inner.input().to_vec();

                // Merge receipt data — no separate UPDATE needed
                let (status, gas_used, contract_created) = receipt_map
                    .get(&tx_hash_str)
                    .map(|r| (
                        r.inner.status(),
                        r.gas_used as i64,
                        r.contract_address.map(|a| format!("{:?}", a)),
                    ))
                    .unwrap_or((false, 0, None));

                batch.t_hashes.push(tx_hash_str.clone());
                batch.t_block_numbers.push(block_num as i64);
                batch.t_block_indices.push(idx as i32);
                batch.t_froms.push(from_str.clone());
                batch.t_tos.push(to_opt.clone());
                batch.t_values.push(value_str);
                batch.t_gas_prices.push(gas_price_str);
                batch.t_gas_used.push(gas_used);
                batch.t_input_data.push(input);
                batch.t_statuses.push(status);
                batch.t_timestamps.push(block.header.timestamp as i64);
                batch.t_contracts_created.push(contract_created.clone());

                batch.tl_hashes.push(tx_hash_str);
                batch.tl_block_numbers.push(block_num as i64);

                // Sender and receiver each get +1 tx_count.
                // Newly created contracts are registered as contracts but don't get a tx_count increment.
                batch.touch_addr(from_str, block_num as i64, false, 1);
                if let Some(to) = to_opt {
                    batch.touch_addr(to, block_num as i64, false, 1);
                }
                if let Some(addr) = contract_created {
                    batch.touch_addr(addr, block_num as i64, true, 0);
                }
            }
        }

        // --- Logs ---
        for receipt in &fetched.receipts {
            for log in receipt.inner.logs() {
                let topics = log.topics();
                let topic0 = match topics.first().map(|t| format!("{:?}", t)) {
                    Some(t) => t,
                    None => continue, // skip logs with no topic0
                };
                let emitter = format!("{:?}", log.address());

                batch.el_tx_hashes.push(log.transaction_hash.map(|h| format!("{:?}", h)).unwrap_or_default());
                batch.el_log_indices.push(log.log_index.unwrap_or(0) as i32);
                batch.el_addresses.push(emitter.clone());
                batch.el_topic0s.push(topic0.clone());
                batch.el_topic1s.push(topics.get(1).map(|t| format!("{:?}", t)));
                batch.el_topic2s.push(topics.get(2).map(|t| format!("{:?}", t)));
                batch.el_topic3s.push(topics.get(3).map(|t| format!("{:?}", t)));
                batch.el_datas.push(log.data().data.to_vec());
                batch.el_block_numbers.push(block_num as i64);

                // Any address that emits logs is a contract
                batch.touch_addr(emitter.clone(), block_num as i64, true, 0);

                if topic0 != TRANSFER_TOPIC {
                    continue;
                }

                match topics.len() {
                    // ERC-721: Transfer(address indexed from, address indexed to, uint256 indexed tokenId)
                    4 => {
                        let contract = emitter.clone();
                        let from = format!("0x{}", hex::encode(&topics[1].as_slice()[12..]));
                        let to = format!("0x{}", hex::encode(&topics[2].as_slice()[12..]));
                        let token_id_str = U256::from_be_slice(topics[3].as_slice()).to_string();

                        if !known_nft.contains(&contract) && batch.new_nft.insert(contract.clone()) {
                            batch.nft_contract_addrs.push(contract.clone());
                            batch.nft_contract_first_seen.push(block_num as i64);
                            batch.touch_addr(contract.clone(), block_num as i64, true, 0);
                        }

                        batch.nt_tx_hashes.push(log.transaction_hash.map(|h| format!("{:?}", h)).unwrap_or_default());
                        batch.nt_log_indices.push(log.log_index.unwrap_or(0) as i32);
                        batch.nt_contracts.push(contract.clone());
                        batch.nt_token_ids.push(token_id_str.clone());
                        batch.nt_froms.push(from);
                        batch.nt_tos.push(to.clone());
                        batch.nt_block_numbers.push(block_num as i64);
                        batch.nt_timestamps.push(block.header.timestamp as i64);

                        // Keep only the latest state per token (last transfer wins)
                        batch.nft_token_map.insert(
                            (contract, token_id_str),
                            NftTokenState { owner: to, last_transfer_block: block_num as i64 },
                        );
                    }
                    // ERC-20: Transfer(address indexed from, address indexed to, uint256 value)
                    3 if log.data().data.len() >= 32 => {
                        let contract = emitter.clone();
                        let from = format!("0x{}", hex::encode(&topics[1].as_slice()[12..]));
                        let to = format!("0x{}", hex::encode(&topics[2].as_slice()[12..]));
                        let value = BigDecimal::from_str(
                            &U256::from_be_slice(&log.data().data[..32]).to_string()
                        ).unwrap_or_default();

                        // Register new contract without blocking RPC calls —
                        // the metadata fetcher will fill in name/symbol/decimals.
                        if !known_erc20.contains(&contract) && batch.new_erc20.insert(contract.clone()) {
                            batch.ec_addresses.push(contract.clone());
                            batch.ec_first_seen_blocks.push(block_num as i64);
                            batch.touch_addr(contract.clone(), block_num as i64, true, 0);
                        }

                        batch.et_tx_hashes.push(log.transaction_hash.map(|h| format!("{:?}", h)).unwrap_or_default());
                        batch.et_log_indices.push(log.log_index.unwrap_or(0) as i32);
                        batch.et_contracts.push(contract.clone());
                        batch.et_froms.push(from.clone());
                        batch.et_tos.push(to.clone());
                        batch.et_values.push(value.to_string());
                        batch.et_block_numbers.push(block_num as i64);
                        batch.et_timestamps.push(block.header.timestamp as i64);

                        // Aggregate balance deltas — multiple transfers in the same batch
                        // for the same (address, contract) pair are summed in Rust,
                        // so we only need one DB upsert per unique pair.
                        if from != ZERO_ADDRESS {
                            batch.apply_balance_delta(from, contract.clone(), -value.clone(), block_num as i64);
                        }
                        if to != ZERO_ADDRESS {
                            batch.apply_balance_delta(to, contract.clone(), value, block_num as i64);
                        }
                    }
                    _ => {}
                }
            }
        }

        batch.last_block = block_num;
    }

    // -----------------------------------------------------------------------
    // write_batch — one DB transaction, one UNNEST query per table.
    // For a batch of N blocks this is ~11 round-trips regardless of N.
    // -----------------------------------------------------------------------

    async fn write_batch(&self, batch: BlockBatch) -> Result<()> {
        if batch.b_numbers.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        // 1. Blocks
        sqlx::query(
            "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
             SELECT * FROM unnest(
                $1::bigint[], $2::text[], $3::text[], $4::bigint[],
                $5::bigint[], $6::bigint[], $7::int[]
             ) AS t(number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count),
             LATERAL (SELECT NOW()) AS ts(indexed_at)
             ON CONFLICT (number) DO UPDATE SET
                hash = EXCLUDED.hash, parent_hash = EXCLUDED.parent_hash,
                timestamp = EXCLUDED.timestamp, gas_used = EXCLUDED.gas_used,
                gas_limit = EXCLUDED.gas_limit, transaction_count = EXCLUDED.transaction_count,
                indexed_at = NOW()"
        )
        .bind(&batch.b_numbers[..])
        .bind(&batch.b_hashes[..])
        .bind(&batch.b_parent_hashes[..])
        .bind(&batch.b_timestamps[..])
        .bind(&batch.b_gas_used[..])
        .bind(&batch.b_gas_limits[..])
        .bind(&batch.b_tx_counts[..])
        .execute(&mut *tx)
        .await?;

        // 2. Transactions (with receipt data already merged — no UPDATE needed)
        if !batch.t_hashes.is_empty() {
            sqlx::query(
                "INSERT INTO transactions
                    (hash, block_number, block_index, from_address, to_address,
                     value, gas_price, gas_used, input_data, status, contract_created, timestamp)
                 SELECT hash, block_number, block_index, from_address, to_address,
                        value::numeric, gas_price::numeric, gas_used, input_data,
                        status, contract_created, timestamp
                 FROM unnest(
                    $1::text[], $2::bigint[], $3::int[], $4::text[], $5::text[],
                    $6::text[], $7::text[], $8::bigint[], $9::bytea[],
                    $10::bool[], $11::text[], $12::bigint[]
                 ) AS t(hash, block_number, block_index, from_address, to_address,
                        value, gas_price, gas_used, input_data,
                        status, contract_created, timestamp)
                 ON CONFLICT (hash, block_number) DO NOTHING"
            )
            .bind(&batch.t_hashes[..])
            .bind(&batch.t_block_numbers[..])
            .bind(&batch.t_block_indices[..])
            .bind(&batch.t_froms[..])
            .bind(&batch.t_tos[..])
            .bind(&batch.t_values[..])
            .bind(&batch.t_gas_prices[..])
            .bind(&batch.t_gas_used[..])
            .bind(&batch.t_input_data[..])
            .bind(&batch.t_statuses[..])
            .bind(&batch.t_contracts_created[..])
            .bind(&batch.t_timestamps[..])
            .execute(&mut *tx)
            .await?;

            // tx_hash_lookup
            sqlx::query(
                "INSERT INTO tx_hash_lookup (hash, block_number)
                 SELECT * FROM unnest($1::text[], $2::bigint[]) AS t(hash, block_number)
                 ON CONFLICT (hash) DO NOTHING"
            )
            .bind(&batch.tl_hashes[..])
            .bind(&batch.tl_block_numbers[..])
            .execute(&mut *tx)
            .await?;
        }

        // 3. Addresses — deduplicated in Rust, one upsert per unique address
        if !batch.addr_map.is_empty() {
            let mut a_addrs: Vec<String> = Vec::with_capacity(batch.addr_map.len());
            let mut a_contracts: Vec<bool> = Vec::with_capacity(batch.addr_map.len());
            let mut a_first_seen: Vec<i64> = Vec::with_capacity(batch.addr_map.len());
            let mut a_tx_counts: Vec<i64> = Vec::with_capacity(batch.addr_map.len());
            for (addr, state) in batch.addr_map {
                a_addrs.push(addr);
                a_contracts.push(state.is_contract);
                a_first_seen.push(state.first_seen_block);
                a_tx_counts.push(state.tx_count_delta);
            }

            sqlx::query(
                "INSERT INTO addresses (address, is_contract, first_seen_block, tx_count)
                 SELECT * FROM unnest($1::text[], $2::bool[], $3::bigint[], $4::bigint[])
                    AS t(address, is_contract, first_seen_block, tx_count)
                 ON CONFLICT (address) DO UPDATE SET
                    tx_count = addresses.tx_count + EXCLUDED.tx_count,
                    is_contract = addresses.is_contract OR EXCLUDED.is_contract,
                    first_seen_block = LEAST(addresses.first_seen_block, EXCLUDED.first_seen_block)"
            )
            .bind(&a_addrs[..])
            .bind(&a_contracts[..])
            .bind(&a_first_seen[..])
            .bind(&a_tx_counts[..])
            .execute(&mut *tx)
            .await?;
        }

        // 4. Event logs
        if !batch.el_tx_hashes.is_empty() {
            sqlx::query(
                "INSERT INTO event_logs
                    (tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number)
                 SELECT * FROM unnest(
                    $1::text[], $2::int[], $3::text[], $4::text[],
                    $5::text[], $6::text[], $7::text[], $8::bytea[], $9::bigint[]
                 ) AS t(tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number)
                 ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING"
            )
            .bind(&batch.el_tx_hashes[..])
            .bind(&batch.el_log_indices[..])
            .bind(&batch.el_addresses[..])
            .bind(&batch.el_topic0s[..])
            .bind(&batch.el_topic1s[..])
            .bind(&batch.el_topic2s[..])
            .bind(&batch.el_topic3s[..])
            .bind(&batch.el_datas[..])
            .bind(&batch.el_block_numbers[..])
            .execute(&mut *tx)
            .await?;
        }

        // 5. NFT contracts
        if !batch.nft_contract_addrs.is_empty() {
            sqlx::query(
                "INSERT INTO nft_contracts (address, first_seen_block)
                 SELECT * FROM unnest($1::text[], $2::bigint[]) AS t(address, first_seen_block)
                 ON CONFLICT (address) DO NOTHING"
            )
            .bind(&batch.nft_contract_addrs[..])
            .bind(&batch.nft_contract_first_seen[..])
            .execute(&mut *tx)
            .await?;
        }

        // 6. NFT transfers
        if !batch.nt_tx_hashes.is_empty() {
            sqlx::query(
                "INSERT INTO nft_transfers
                    (tx_hash, log_index, contract_address, token_id, from_address, to_address, block_number, timestamp)
                 SELECT tx_hash, log_index, contract_address, token_id::numeric,
                        from_address, to_address, block_number, timestamp
                 FROM unnest(
                    $1::text[], $2::int[], $3::text[], $4::text[],
                    $5::text[], $6::text[], $7::bigint[], $8::bigint[]
                 ) AS t(tx_hash, log_index, contract_address, token_id,
                        from_address, to_address, block_number, timestamp)
                 ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING"
            )
            .bind(&batch.nt_tx_hashes[..])
            .bind(&batch.nt_log_indices[..])
            .bind(&batch.nt_contracts[..])
            .bind(&batch.nt_token_ids[..])
            .bind(&batch.nt_froms[..])
            .bind(&batch.nt_tos[..])
            .bind(&batch.nt_block_numbers[..])
            .bind(&batch.nt_timestamps[..])
            .execute(&mut *tx)
            .await?;
        }

        // 7. NFT tokens — only the latest state per token
        if !batch.nft_token_map.is_empty() {
            let mut tok_contracts: Vec<String> = Vec::with_capacity(batch.nft_token_map.len());
            let mut tok_ids: Vec<String> = Vec::with_capacity(batch.nft_token_map.len());
            let mut tok_owners: Vec<String> = Vec::with_capacity(batch.nft_token_map.len());
            let mut tok_last_blocks: Vec<i64> = Vec::with_capacity(batch.nft_token_map.len());
            for ((contract, token_id), state) in batch.nft_token_map {
                tok_contracts.push(contract);
                tok_ids.push(token_id);
                tok_owners.push(state.owner);
                tok_last_blocks.push(state.last_transfer_block);
            }

            sqlx::query(
                "INSERT INTO nft_tokens (contract_address, token_id, owner, metadata_fetched, last_transfer_block)
                 SELECT contract_address, token_id::numeric, owner, false, last_transfer_block
                 FROM unnest($1::text[], $2::text[], $3::text[], $4::bigint[])
                    AS t(contract_address, token_id, owner, last_transfer_block)
                 ON CONFLICT (contract_address, token_id) DO UPDATE SET
                    owner = CASE
                        WHEN EXCLUDED.last_transfer_block >= nft_tokens.last_transfer_block
                        THEN EXCLUDED.owner
                        ELSE nft_tokens.owner
                    END,
                    last_transfer_block = GREATEST(nft_tokens.last_transfer_block, EXCLUDED.last_transfer_block)"
            )
            .bind(&tok_contracts[..])
            .bind(&tok_ids[..])
            .bind(&tok_owners[..])
            .bind(&tok_last_blocks[..])
            .execute(&mut *tx)
            .await?;
        }

        // 8. ERC-20 contracts (new only, without metadata)
        if !batch.ec_addresses.is_empty() {
            sqlx::query(
                "INSERT INTO erc20_contracts (address, decimals, first_seen_block)
                 SELECT address, 18, first_seen_block
                 FROM unnest($1::text[], $2::bigint[]) AS t(address, first_seen_block)
                 ON CONFLICT (address) DO NOTHING"
            )
            .bind(&batch.ec_addresses[..])
            .bind(&batch.ec_first_seen_blocks[..])
            .execute(&mut *tx)
            .await?;
        }

        // 9. ERC-20 transfers
        if !batch.et_tx_hashes.is_empty() {
            sqlx::query(
                "INSERT INTO erc20_transfers
                    (tx_hash, log_index, contract_address, from_address, to_address, value, block_number, timestamp)
                 SELECT tx_hash, log_index, contract_address, from_address, to_address,
                        value::numeric, block_number, timestamp
                 FROM unnest(
                    $1::text[], $2::int[], $3::text[], $4::text[], $5::text[],
                    $6::text[], $7::bigint[], $8::bigint[]
                 ) AS t(tx_hash, log_index, contract_address, from_address, to_address,
                        value, block_number, timestamp)
                 ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING"
            )
            .bind(&batch.et_tx_hashes[..])
            .bind(&batch.et_log_indices[..])
            .bind(&batch.et_contracts[..])
            .bind(&batch.et_froms[..])
            .bind(&batch.et_tos[..])
            .bind(&batch.et_values[..])
            .bind(&batch.et_block_numbers[..])
            .bind(&batch.et_timestamps[..])
            .execute(&mut *tx)
            .await?;
        }

        // 10. ERC-20 balance deltas — aggregated in Rust, one upsert per unique (address, contract)
        if !batch.balance_map.is_empty() {
            let mut bal_addrs: Vec<String> = Vec::with_capacity(batch.balance_map.len());
            let mut bal_contracts: Vec<String> = Vec::with_capacity(batch.balance_map.len());
            let mut bal_deltas: Vec<String> = Vec::with_capacity(batch.balance_map.len());
            let mut bal_blocks: Vec<i64> = Vec::with_capacity(batch.balance_map.len());
            for ((addr, contract), delta) in batch.balance_map {
                bal_addrs.push(addr);
                bal_contracts.push(contract);
                bal_deltas.push(delta.delta.to_string());
                bal_blocks.push(delta.last_block);
            }

            sqlx::query(
                "INSERT INTO erc20_balances (address, contract_address, balance, last_updated_block)
                 SELECT address, contract_address, balance::numeric, last_updated_block
                 FROM unnest($1::text[], $2::text[], $3::text[], $4::bigint[])
                    AS t(address, contract_address, balance, last_updated_block)
                 ON CONFLICT (address, contract_address) DO UPDATE SET
                    balance = erc20_balances.balance + EXCLUDED.balance,
                    last_updated_block = GREATEST(erc20_balances.last_updated_block, EXCLUDED.last_updated_block)"
            )
            .bind(&bal_addrs[..])
            .bind(&bal_contracts[..])
            .bind(&bal_deltas[..])
            .bind(&bal_blocks[..])
            .execute(&mut *tx)
            .await?;
        }

        // 11. Indexer state — once per batch instead of once per block
        sqlx::query(
            "INSERT INTO indexer_state (key, value, updated_at)
             VALUES ('last_indexed_block', $1, NOW())
             ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW()"
        )
        .bind(batch.last_block.to_string())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    async fn load_known_erc20(&self) -> Result<HashSet<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT address FROM erc20_contracts")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(a,)| a).collect())
    }

    async fn load_known_nft(&self) -> Result<HashSet<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT address FROM nft_contracts")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(a,)| a).collect())
    }

    async fn get_start_block(&self) -> Result<u64> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM indexer_state WHERE key = 'last_indexed_block'"
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some((value,)) = result {
            let last_block: u64 = value.parse()?;
            Ok(last_block + 1)
        } else {
            Ok(self.config.start_block)
        }
    }

    async fn fetch_blocks_batch(
        client: &reqwest::Client,
        rpc_url: &str,
        start_block: u64,
        count: usize,
        rate_limiter: &SharedRateLimiter,
    ) -> Vec<FetchResult> {
        tracing::debug!("Fetching batch: blocks {} to {}", start_block, start_block + count as u64 - 1);

        // Wait for rate limiter - we're making 2*count RPC calls in one HTTP request
        for _ in 0..(count * 2) {
            rate_limiter.until_ready().await;
        }

        // Build batch request: eth_getBlockByNumber + eth_getBlockReceipts per block
        let mut batch_request = Vec::with_capacity(count * 2);
        for i in 0..count {
            let block_num = start_block + i as u64;
            let block_hex = format!("0x{:x}", block_num);

            // eth_getBlockByNumber with full transactions
            batch_request.push(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_getBlockByNumber",
                "params": [block_hex, true],
                "id": i * 2
            }));

            // eth_getBlockReceipts
            batch_request.push(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_getBlockReceipts",
                "params": [block_hex],
                "id": i * 2 + 1
            }));
        }

        // Send batch request with retry for network errors
        let mut batch_response: Option<Vec<serde_json::Value>> = None;
        let mut last_error: Option<String> = None;

        for attempt in 0..RPC_MAX_RETRIES {
            // Send request
            let response = match client
                .post(rpc_url)
                .json(&batch_request)
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    let delay = RPC_RETRY_DELAYS
                        .get(attempt)
                        .copied()
                        .unwrap_or(*RPC_RETRY_DELAYS.last().unwrap_or(&30));

                    tracing::warn!(
                        "RPC batch request failed (attempt {}/{}): {}. Retrying in {}s...",
                        attempt + 1,
                        RPC_MAX_RETRIES,
                        e,
                        delay
                    );

                    last_error = Some(format!("HTTP request failed: {}", e));
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    continue;
                }
            };

            // Parse response
            match response.json::<Vec<serde_json::Value>>().await {
                Ok(resp) => {
                    if attempt > 0 {
                        tracing::info!(
                            "RPC batch request succeeded after {} retries (blocks {} to {})",
                            attempt,
                            start_block,
                            start_block + count as u64 - 1
                        );
                    }
                    batch_response = Some(resp);
                    break;
                }
                Err(e) => {
                    let delay = RPC_RETRY_DELAYS
                        .get(attempt)
                        .copied()
                        .unwrap_or(*RPC_RETRY_DELAYS.last().unwrap_or(&30));

                    tracing::warn!(
                        "Failed to parse RPC response (attempt {}/{}): {}. Retrying in {}s...",
                        attempt + 1,
                        RPC_MAX_RETRIES,
                        e,
                        delay
                    );

                    last_error = Some(format!("Failed to parse response: {}", e));
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
            }
        }

        // If all retries failed, return errors for all blocks
        let batch_response = match batch_response {
            Some(resp) => resp,
            None => {
                let error_msg = last_error.unwrap_or_else(|| "Unknown error".to_string());
                return (0..count)
                    .map(|i| FetchResult::Error {
                        block_num: start_block + i as u64,
                        error: error_msg.clone(),
                    })
                    .collect();
            }
        };

        // Process responses - they should be in order by ID
        let mut results = Vec::with_capacity(count);
        let mut response_map: BTreeMap<u64, &serde_json::Value> = BTreeMap::new();

        for resp in &batch_response {
            if let Some(id) = resp.get("id").and_then(|v| v.as_u64()) {
                response_map.insert(id, resp);
            }
        }

        for i in 0..count {
            let block_num = start_block + i as u64;
            let block_id = (i * 2) as u64;
            let receipts_id = (i * 2 + 1) as u64;

            // Get block response
            let block_result = match response_map.get(&block_id) {
                Some(resp) => {
                    if let Some(error) = resp.get("error") {
                        Err(format!("RPC error: {}", error))
                    } else if let Some(result) = resp.get("result") {
                        if result.is_null() {
                            Err(format!("Block {} not found", block_num))
                        } else {
                            serde_json::from_value::<Block>(result.clone())
                                .map_err(|e| format!("Failed to parse block: {}", e))
                        }
                    } else {
                        Err("No result in response".to_string())
                    }
                }
                None => Err(format!("Missing response for block {}", block_num)),
            };

            // Get receipts response
            let receipts_result = match response_map.get(&receipts_id) {
                Some(resp) => {
                    if let Some(error) = resp.get("error") {
                        Err(format!("RPC error: {}", error))
                    } else if let Some(result) = resp.get("result") {
                        if result.is_null() {
                            Ok(Vec::new())
                        } else {
                            serde_json::from_value::<Vec<TransactionReceipt>>(result.clone())
                                .map_err(|e| format!("Failed to parse receipts: {}", e))
                        }
                    } else {
                        Ok(Vec::new())
                    }
                }
                None => Ok(Vec::new()),
            };

            // Combine block + receipts into a single result
            match (block_result, receipts_result) {
                (Ok(block), Ok(receipts)) => {
                    tracing::debug!("Block {} complete ({} receipts)", block_num, receipts.len());
                    results.push(FetchResult::Success(FetchedBlock {
                        number: block_num,
                        block,
                        receipts,
                    }));
                }
                (Err(e), _) => {
                    tracing::warn!("Failed to fetch block {}: {}", block_num, e);
                    results.push(FetchResult::Error {
                        block_num,
                        error: e,
                    });
                }
                (_, Err(e)) => {
                    tracing::warn!("Failed to fetch receipts for block {}: {}", block_num, e);
                    results.push(FetchResult::Error {
                        block_num,
                        error: e,
                    });
                }
            }
        }

        results
    }

    async fn ensure_partitions_exist(&self, block_number: u64) -> Result<()> {
        use std::sync::atomic::Ordering;

        let partition_num = block_number / PARTITION_SIZE;
        let current_max = self.current_max_partition.load(Ordering::Relaxed);

        // Fast path: partition already exists (most common case)
        if partition_num <= current_max {
            return Ok(());
        }

        // First run or crossing boundary - need to check/create partitions
        // Create all partitions from current_max+1 to partition_num
        let start_partition = if current_max == 0 {
            // First run - check what partitions exist
            let existing: Option<(i64,)> = sqlx::query_as(
                "SELECT MAX(CAST(SUBSTRING(relname FROM 'blocks_p(\\d+)') AS BIGINT))
                 FROM pg_class WHERE relname ~ '^blocks_p\\d+$'"
            )
            .fetch_optional(&self.pool)
            .await?;

            match existing {
                Some((max,)) => {
                    self.current_max_partition.store(max as u64, Ordering::Relaxed);
                    if partition_num <= max as u64 {
                        return Ok(());
                    }
                    max as u64 + 1
                }
                None => 0, // No partitions exist, start from 0
            }
        } else {
            current_max + 1
        };

        // Create any missing partitions
        for p in start_partition..=partition_num {
            let partition_start = p * PARTITION_SIZE;
            let partition_end = partition_start + PARTITION_SIZE;

            tracing::info!(
                "Creating partitions for block range {} to {} (p{})",
                partition_start,
                partition_end,
                p
            );

            // Create partitions for all partitioned tables
            let tables = [
                "blocks",
                "transactions",
                "event_logs",
                "nft_transfers",
                "erc20_transfers",
            ];

            for table in tables {
                let create_sql = format!(
                    "CREATE TABLE IF NOT EXISTS {}_p{} PARTITION OF {} FOR VALUES FROM ({}) TO ({})",
                    table, p, table, partition_start, partition_end
                );

                sqlx::query(&create_sql)
                    .execute(&self.pool)
                    .await?;
            }
        }

        // Update our tracked max
        self.current_max_partition.store(partition_num, Ordering::Relaxed);
        tracing::info!("Partitions up to p{} ready", partition_num);
        Ok(())
    }

    /// Get block number with internal retry logic for network failures
    async fn get_block_number_with_retry(&self, provider: &HttpProvider) -> Result<u64> {
        let mut last_error = None;

        for attempt in 0..RPC_MAX_RETRIES {
            match provider.get_block_number().await {
                Ok(block_num) => {
                    if attempt > 0 {
                        tracing::info!("RPC connection restored after {} retries", attempt);
                    }
                    return Ok(block_num);
                }
                Err(e) => {
                    let delay = RPC_RETRY_DELAYS
                        .get(attempt)
                        .copied()
                        .unwrap_or(*RPC_RETRY_DELAYS.last().unwrap_or(&30));

                    tracing::warn!(
                        "RPC request failed (attempt {}/{}): {}. Retrying in {}s...",
                        attempt + 1,
                        RPC_MAX_RETRIES,
                        e,
                        delay
                    );

                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
            }
        }

        Err(anyhow::anyhow!(
            "RPC connection failed after {} retries: {:?}",
            RPC_MAX_RETRIES,
            last_error
        ))
    }

    async fn truncate_tables(&self) -> Result<()> {
        sqlx::query(
            "TRUNCATE blocks, transactions, addresses, nft_contracts, nft_tokens, nft_transfers,
             erc20_contracts, erc20_transfers, erc20_balances, event_logs, proxy_contracts, indexer_state CASCADE"
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
