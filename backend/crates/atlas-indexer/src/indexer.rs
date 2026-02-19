use alloy::network::Ethereum;
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::types::{Block, Log, TransactionReceipt};
use alloy::transports::http::{Client, Http};
use anyhow::Result;
use bigdecimal::BigDecimal;
use governor::{Quota, RateLimiter};
use sqlx::PgPool;
use std::collections::BTreeMap;
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

/// Work item for a worker - a range of blocks to fetch
#[derive(Debug, Clone)]
struct WorkItem {
    start_block: u64,
    count: usize,
}

/// ERC-20/721 Transfer event signature: Transfer(address,address,uint256)
const TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

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

        let num_workers = self.config.fetch_workers as usize;
        let rpc_batch_size = self.config.rpc_batch_size as usize;
        tracing::info!("Starting {} fetch workers with {} blocks per RPC batch", num_workers, rpc_batch_size);

        // Channels for work distribution and results
        // work_tx: send WorkItems (block ranges) to fetch
        // result_tx: workers send fetched blocks back
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

        // Drop our copy of result_tx so channel closes when all workers done
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

            // Collect results with reorder buffer
            let mut buffer: BTreeMap<u64, FetchedBlock> = BTreeMap::new();
            let mut next_to_process = current_block;
            let mut blocks_received = 0;
            let mut failed_blocks: Vec<(u64, String)> = Vec::new();

            // Receive all blocks for this batch
            while blocks_received < batch_size {
                match result_rx.recv().await {
                    Some(FetchResult::Success(fetched)) => {
                        buffer.insert(fetched.number, fetched);
                        blocks_received += 1;

                        // Process all consecutive blocks we have in order
                        while let Some(data) = buffer.remove(&next_to_process) {
                            self.process_block(&provider, data).await?;
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

            // Wait for send task to complete
            let _ = send_task.await;

            // Retry failed blocks if any
            if !failed_blocks.is_empty() {
                let block_nums: Vec<u64> = failed_blocks.iter().map(|(n, _)| *n).collect();
                tracing::warn!("Retrying {} failed blocks: {:?}", failed_blocks.len(), block_nums);

                // Retry up to 3 times with increasing delay
                for attempt in 1..=3 {
                    if failed_blocks.is_empty() {
                        break;
                    }

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
                                // Process the retried block
                                self.process_block(&provider, fetched).await?;
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

    /// Fetch multiple blocks using JSON-RPC batch request
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

        // Build batch request
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

            // Combine results
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

    /// Process a fetched block (runs sequentially in main loop)
    async fn process_block(&self, provider: &HttpProvider, fetched: FetchedBlock) -> Result<()> {
        let block = fetched.block;
        let block_num = fetched.number;

        let mut tx = self.pool.begin().await?;

        // Insert block
        let tx_count = block.transactions.len() as i32;

        sqlx::query(
            "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
             ON CONFLICT (number) DO UPDATE SET
                hash = $2, parent_hash = $3, timestamp = $4, gas_used = $5, gas_limit = $6,
                transaction_count = $7, indexed_at = NOW()"
        )
        .bind(block.header.number as i64)
        .bind(format!("{:?}", block.header.hash))
        .bind(format!("{:?}", block.header.parent_hash))
        .bind(block.header.timestamp as i64)
        .bind(block.header.gas_used as i64)
        .bind(block.header.gas_limit as i64)
        .bind(tx_count)
        .execute(&mut *tx)
        .await?;

        // Process transactions
        if let Some(txs) = block.transactions.as_transactions() {
            for (idx, transaction) in txs.iter().enumerate() {
                self.insert_transaction(&mut tx, transaction, block_num, idx as i32, block.header.timestamp).await?;
            }
        }

        // Process receipts and logs
        for receipt in fetched.receipts {
            // Update the transaction row with accurate receipt data
            {
                // Extract tx hash (field on TransactionReceipt)
                let tx_hash_str = format!("{:?}", receipt.transaction_hash);

                // Status from inner receipt (via TxReceipt trait)
                let status_flag: bool = receipt.inner.status();

                // Gas used (field on TransactionReceipt)
                let gas_used_i64: i64 = receipt.gas_used as i64;

                // Contract address (if contract creation)
                let created_addr: Option<String> = receipt
                    .contract_address
                    .map(|a| format!("{:?}", a));

                sqlx::query(
                    "UPDATE transactions SET status = $1, gas_used = $2, contract_created = $3
                     WHERE hash = $4 AND block_number = $5"
                )
                .bind(status_flag)
                .bind(gas_used_i64)
                .bind(&created_addr)
                .bind(&tx_hash_str)
                .bind(block_num as i64)
                .execute(&mut *tx)
                .await?;

                // Mark newly created contract as contract address
                if let Some(ref addr) = &created_addr {
                    self.mark_address_as_contract(&mut tx, addr, block_num).await?;
                }
            }

            for log in receipt.inner.logs() {
                // Store all event logs
                self.insert_event_log(&mut tx, log, block_num).await?;

                // Process Transfer events
                if self.is_transfer_event(log) {
                    if self.is_erc721_transfer(log) {
                        self.process_nft_transfer(&mut tx, log, block_num, block.header.timestamp).await?;
                    } else if self.is_erc20_transfer(log) {
                        self.process_erc20_transfer(&mut tx, provider, log, block_num, block.header.timestamp).await?;
                    }
                }
            }
        }

        // Update indexer state
        sqlx::query(
            "INSERT INTO indexer_state (key, value, updated_at)
             VALUES ('last_indexed_block', $1, NOW())
             ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW()"
        )
        .bind(block_num.to_string())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn mark_address_as_contract(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        address_str: &str,
        first_seen_block: u64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO addresses (address, is_contract, first_seen_block, tx_count)
             VALUES ($1, true, $2, 0)
             ON CONFLICT (address) DO UPDATE SET
                is_contract = true,
                first_seen_block = LEAST(addresses.first_seen_block, $2)"
        )
        .bind(address_str)
        .bind(first_seen_block as i64)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn get_start_block(&self) -> Result<u64> {
        // Check for last indexed block
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

    /// Ensure partitions exist for all partitioned tables up to the given block number
    /// Uses in-memory tracking to avoid database queries on every batch
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

    async fn insert_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        transaction: &alloy::rpc::types::Transaction,
        block_number: u64,
        block_index: i32,
        timestamp: u64,
    ) -> Result<()> {
        use alloy::consensus::Transaction as TxTrait;

        // Access transaction fields through inner
        let inner = &transaction.inner;
        let tx_hash = inner.tx_hash();
        let value = inner.value();
        let gas_limit = inner.gas_limit();
        let to_addr = inner.to();
        let input = inner.input();
        let from_addr = transaction.from;

        let value_decimal = BigDecimal::from_str(&value.to_string())?;
        let gas_price = transaction.effective_gas_price
            .map(|gp| BigDecimal::from_str(&gp.to_string()))
            .transpose()?
            .unwrap_or_else(|| BigDecimal::from(0));

        let tx_hash_str = format!("{:?}", tx_hash);

        sqlx::query(
            "INSERT INTO transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (hash, block_number) DO NOTHING"
        )
        .bind(&tx_hash_str)
        .bind(block_number as i64)
        .bind(block_index)
        .bind(format!("{:?}", from_addr))
        .bind(to_addr.map(|a| format!("{:?}", a)))
        .bind(value_decimal)
        .bind(gas_price)
        .bind(0i64) // gas_used default; updated later from receipt
        .bind(input.to_vec())
        .bind(false) // status default; updated later from receipt
        .bind(timestamp as i64)
        .execute(&mut **tx)
        .await?;

        // Insert into hash lookup table for fast search
        sqlx::query(
            "INSERT INTO tx_hash_lookup (hash, block_number)
             VALUES ($1, $2)
             ON CONFLICT (hash) DO NOTHING"
        )
        .bind(&tx_hash_str)
        .bind(block_number as i64)
        .execute(&mut **tx)
        .await?;

        // Upsert addresses
        self.upsert_address(tx, from_addr, block_number, false).await?;
        if let Some(to) = to_addr {
            // Do not infer contract status from calldata; just upsert address and tx_count
            self.upsert_address(tx, to, block_number, false).await?;
        }

        Ok(())
    }

    async fn upsert_address(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        address: Address,
        block_number: u64,
        is_contract: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO addresses (address, is_contract, first_seen_block, tx_count)
             VALUES ($1, $2, $3, 1)
             ON CONFLICT (address) DO UPDATE SET
                tx_count = addresses.tx_count + 1,
                is_contract = addresses.is_contract OR $2"
        )
        .bind(format!("{:?}", address))
        .bind(is_contract)
        .bind(block_number as i64)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    fn is_transfer_event(&self, log: &Log) -> bool {
        if log.topics().is_empty() {
            return false;
        }
        let topic0 = format!("{:?}", log.topics()[0]);
        topic0 == TRANSFER_TOPIC
    }

    fn is_erc721_transfer(&self, log: &Log) -> bool {
        // ERC-721: 4 topics (signature + from + to + tokenId)
        if log.topics().len() != 4 {
            return false;
        }
        let topic0 = format!("{:?}", log.topics()[0]);
        topic0 == TRANSFER_TOPIC
    }

    fn is_erc20_transfer(&self, log: &Log) -> bool {
        // ERC-20: 3 topics (signature + from + to) with value in data
        if log.topics().len() != 3 {
            return false;
        }
        let topic0 = format!("{:?}", log.topics()[0]);
        topic0 == TRANSFER_TOPIC && log.data().data.len() >= 32
    }

    async fn process_nft_transfer(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        log: &Log,
        block_number: u64,
        timestamp: u64,
    ) -> Result<()> {
        let contract_address = format!("{:?}", log.address());
        let from_address = format!("0x{}", hex::encode(&log.topics()[1].as_slice()[12..]));
        let to_address = format!("0x{}", hex::encode(&log.topics()[2].as_slice()[12..]));
        let token_id = U256::from_be_slice(log.topics()[3].as_slice());
        let token_id_decimal = BigDecimal::from_str(&token_id.to_string())?;

        // Upsert NFT contract
        sqlx::query(
            "INSERT INTO nft_contracts (address, first_seen_block)
             VALUES ($1, $2)
             ON CONFLICT (address) DO NOTHING"
        )
        .bind(&contract_address)
        .bind(block_number as i64)
        .execute(&mut **tx)
        .await?;

        // Mark as contract
        self.mark_address_as_contract(tx, &contract_address, block_number).await?;

        // Insert transfer record
        sqlx::query(
            "INSERT INTO nft_transfers (tx_hash, log_index, contract_address, token_id, from_address, to_address, block_number, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        )
        .bind(log.transaction_hash.map(|h| format!("{:?}", h)).unwrap_or_default())
        .bind(log.log_index.unwrap_or(0) as i32)
        .bind(&contract_address)
        .bind(&token_id_decimal)
        .bind(&from_address)
        .bind(&to_address)
        .bind(block_number as i64)
        .bind(timestamp as i64)
        .execute(&mut **tx)
        .await?;

        // Upsert NFT token (update owner)
        sqlx::query(
            "INSERT INTO nft_tokens (contract_address, token_id, owner, metadata_fetched, last_transfer_block)
             VALUES ($1, $2, $3, false, $4)
             ON CONFLICT (contract_address, token_id) DO UPDATE SET
                owner = $3,
                last_transfer_block = $4"
        )
        .bind(&contract_address)
        .bind(&token_id_decimal)
        .bind(&to_address)
        .bind(block_number as i64)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    /// Insert event log into database
    async fn insert_event_log(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        log: &Log,
        block_number: u64,
    ) -> Result<()> {
        let topics = log.topics();
        let topic0 = topics.first().map(|t| format!("{:?}", t));
        let topic1 = topics.get(1).map(|t| format!("{:?}", t));
        let topic2 = topics.get(2).map(|t| format!("{:?}", t));
        let topic3 = topics.get(3).map(|t| format!("{:?}", t));

        // Skip if no topic0 (invalid log)
        let topic0 = match topic0 {
            Some(t) => t,
            None => return Ok(()),
        };

        sqlx::query(
            "INSERT INTO event_logs (tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING"
        )
        .bind(log.transaction_hash.map(|h| format!("{:?}", h)).unwrap_or_default())
        .bind(log.log_index.unwrap_or(0) as i32)
        .bind(format!("{:?}", log.address()))
        .bind(&topic0)
        .bind(topic1)
        .bind(topic2)
        .bind(topic3)
        .bind(log.data().data.to_vec())
        .bind(block_number as i64)
        .execute(&mut **tx)
        .await?;

        // Any address that emits logs is necessarily a contract
        let emitter = format!("{:?}", log.address());
        self.mark_address_as_contract(tx, &emitter, block_number).await?;

        Ok(())
    }

    /// Process ERC-20 Transfer event
    async fn process_erc20_transfer(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        provider: &HttpProvider,
        log: &Log,
        block_number: u64,
        timestamp: u64,
    ) -> Result<()> {
        let contract_address = format!("{:?}", log.address());
        let from_address = format!("0x{}", hex::encode(&log.topics()[1].as_slice()[12..]));
        let to_address = format!("0x{}", hex::encode(&log.topics()[2].as_slice()[12..]));

        // Parse value from data (first 32 bytes)
        let log_data = log.data();
        let value = if log_data.data.len() >= 32 {
            U256::from_be_slice(&log_data.data[..32])
        } else {
            U256::ZERO
        };
        let value_decimal = BigDecimal::from_str(&value.to_string())?;

        // Check if contract exists, if not fetch metadata
        let exists: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM erc20_contracts WHERE LOWER(address) = LOWER($1)"
        )
        .bind(&contract_address)
        .fetch_optional(&mut **tx)
        .await?;

        if exists.is_none() {
            // Fetch ERC-20 metadata from contract
            let (name, symbol, decimals, total_supply) = self.fetch_erc20_metadata(provider, log.address()).await;

            sqlx::query(
                "INSERT INTO erc20_contracts (address, name, symbol, decimals, total_supply, first_seen_block)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (address) DO NOTHING"
            )
            .bind(&contract_address)
            .bind(name)
            .bind(symbol)
            .bind(decimals)
            .bind(total_supply)
            .bind(block_number as i64)
            .execute(&mut **tx)
            .await?;

            // Mark as contract
            self.mark_address_as_contract(tx, &contract_address, block_number).await?;
        }

        // Insert transfer record
        sqlx::query(
            "INSERT INTO erc20_transfers (tx_hash, log_index, contract_address, from_address, to_address, value, block_number, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING"
        )
        .bind(log.transaction_hash.map(|h| format!("{:?}", h)).unwrap_or_default())
        .bind(log.log_index.unwrap_or(0) as i32)
        .bind(&contract_address)
        .bind(&from_address)
        .bind(&to_address)
        .bind(&value_decimal)
        .bind(block_number as i64)
        .bind(timestamp as i64)
        .execute(&mut **tx)
        .await?;

        // Update balances
        // Decrease from_address balance (if not zero address)
        if from_address != "0x0000000000000000000000000000000000000000" {
            sqlx::query(
                "INSERT INTO erc20_balances (address, contract_address, balance, last_updated_block)
                 VALUES ($1, $2, -$3, $4)
                 ON CONFLICT (address, contract_address) DO UPDATE SET
                    balance = erc20_balances.balance - $3,
                    last_updated_block = $4"
            )
            .bind(&from_address)
            .bind(&contract_address)
            .bind(&value_decimal)
            .bind(block_number as i64)
            .execute(&mut **tx)
            .await?;
        }

        // Increase to_address balance (if not zero address)
        if to_address != "0x0000000000000000000000000000000000000000" {
            sqlx::query(
                "INSERT INTO erc20_balances (address, contract_address, balance, last_updated_block)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (address, contract_address) DO UPDATE SET
                    balance = erc20_balances.balance + $3,
                    last_updated_block = $4"
            )
            .bind(&to_address)
            .bind(&contract_address)
            .bind(&value_decimal)
            .bind(block_number as i64)
            .execute(&mut **tx)
            .await?;
        }

        // Update totalSupply if this is a mint or burn
        let is_mint = from_address == "0x0000000000000000000000000000000000000000";
        let is_burn = to_address == "0x0000000000000000000000000000000000000000";

        if is_mint || is_burn {
            // Re-fetch totalSupply from the contract
            const TOTAL_SUPPLY_SELECTOR: [u8; 4] = [0x18, 0x16, 0x0d, 0xdd];
            let new_supply = self.call_uint256_method(provider, log.address(), &TOTAL_SUPPLY_SELECTOR).await;

            sqlx::query(
                "UPDATE erc20_contracts SET total_supply = $1 WHERE LOWER(address) = LOWER($2)"
            )
            .bind(new_supply)
            .bind(&contract_address)
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }

    /// Fetch ERC-20 metadata (name, symbol, decimals) from contract
    async fn fetch_erc20_metadata(
        &self,
        provider: &HttpProvider,
        address: Address,
    ) -> (Option<String>, Option<String>, i16, Option<bigdecimal::BigDecimal>) {
        // Function selectors
        const NAME_SELECTOR: [u8; 4] = [0x06, 0xfd, 0xde, 0x03]; // name()
        const SYMBOL_SELECTOR: [u8; 4] = [0x95, 0xd8, 0x9b, 0x41]; // symbol()
        const DECIMALS_SELECTOR: [u8; 4] = [0x31, 0x3c, 0xe5, 0x67]; // decimals()
        const TOTAL_SUPPLY_SELECTOR: [u8; 4] = [0x18, 0x16, 0x0d, 0xdd]; // totalSupply()

        let name = self.call_string_method(provider, address, &NAME_SELECTOR).await;
        let symbol = self.call_string_method(provider, address, &SYMBOL_SELECTOR).await;
        let decimals = self.call_uint8_method(provider, address, &DECIMALS_SELECTOR).await.unwrap_or(18);
        let total_supply = self.call_uint256_method(provider, address, &TOTAL_SUPPLY_SELECTOR).await;

        (name, symbol, decimals as i16, total_supply)
    }

    /// Call a method that returns a string
    async fn call_string_method(
        &self,
        provider: &HttpProvider,
        address: Address,
        selector: &[u8; 4],
    ) -> Option<String> {
        use alloy::rpc::types::TransactionRequest;

        let tx = TransactionRequest::default()
            .to(address)
            .input(alloy::primitives::Bytes::from(selector.to_vec()).into());

        let result = provider.call(&tx).await.ok()?;

        // Decode string from ABI encoding
        if result.len() < 64 {
            return None;
        }

        // Offset is at bytes 0-32
        let offset = U256::from_be_slice(&result[0..32]).to::<usize>();
        if offset + 32 > result.len() {
            return None;
        }

        // Length is at offset position
        let length = U256::from_be_slice(&result[offset..offset + 32]).to::<usize>();
        if offset + 32 + length > result.len() {
            return None;
        }

        // String data follows
        let string_data = &result[offset + 32..offset + 32 + length];
        String::from_utf8(string_data.to_vec()).ok()
    }

    /// Call a method that returns a uint8
    async fn call_uint8_method(
        &self,
        provider: &HttpProvider,
        address: Address,
        selector: &[u8; 4],
    ) -> Option<u8> {
        use alloy::rpc::types::TransactionRequest;

        let tx = TransactionRequest::default()
            .to(address)
            .input(alloy::primitives::Bytes::from(selector.to_vec()).into());

        let result = provider.call(&tx).await.ok()?;

        if result.len() < 32 {
            return None;
        }

        // uint8 is right-padded in 32 bytes
        Some(result[31])
    }

    /// Call a method that returns a uint256
    async fn call_uint256_method(
        &self,
        provider: &HttpProvider,
        address: Address,
        selector: &[u8; 4],
    ) -> Option<bigdecimal::BigDecimal> {
        use alloy::rpc::types::TransactionRequest;
        use bigdecimal::BigDecimal;
        use num_bigint::BigInt;

        let tx = TransactionRequest::default()
            .to(address)
            .input(alloy::primitives::Bytes::from(selector.to_vec()).into());

        let result = provider.call(&tx).await.ok()?;

        if result.len() < 32 {
            return None;
        }

        // uint256 is a 32-byte big-endian value
        let value = BigInt::from_bytes_be(num_bigint::Sign::Plus, &result[0..32]);
        Some(BigDecimal::from(value))
    }
}
