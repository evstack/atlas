use alloy::network::Ethereum;
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::types::{BlockId, BlockNumberOrTag, BlockTransactionsKind, Log};
use alloy::transports::http::{Client, Http};
use anyhow::Result;
use bigdecimal::BigDecimal;
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;

use crate::config::Config;

/// ERC-20/721 Transfer event signature: Transfer(address,address,uint256)
/// Same signature for both, differentiated by topic count:
/// - ERC-721: 4 topics (topic0 + from + to + tokenId)
/// - ERC-20: 3 topics (topic0 + from + to) with value in data
const TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

type HttpProvider = RootProvider<Http<Client>, Ethereum>;

pub struct Indexer {
    pool: PgPool,
    config: Config,
}

impl Indexer {
    pub fn new(pool: PgPool, config: Config) -> Self {
        Self { pool, config }
    }

    pub async fn run(&self) -> Result<()> {
        let provider = ProviderBuilder::new().on_http(self.config.rpc_url.parse()?);

        // Handle reindex flag
        if self.config.reindex {
            tracing::warn!("Reindex flag set - truncating all tables");
            self.truncate_tables().await?;
        }

        // Get starting block
        let start_block = self.get_start_block().await?;
        tracing::info!("Starting indexing from block {}", start_block);

        let mut current_block = start_block;
        let rate_limit_delay = Duration::from_millis(1000 / self.config.rpc_requests_per_second as u64);
        let mut last_logged_block: u64 = start_block;
        let mut last_log_time = std::time::Instant::now();

        loop {
            // Get chain head
            let head = provider.get_block_number().await?;

            if current_block > head {
                // At head, wait for new blocks
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }

            // Index blocks in batches
            let end_block = (current_block + self.config.batch_size - 1).min(head);

            for block_num in current_block..=end_block {
                self.index_block(&provider, block_num).await?;
                tokio::time::sleep(rate_limit_delay).await;
            }

            current_block = end_block + 1;

            // Log progress every 1000 blocks
            if end_block / 1000 > last_logged_block / 1000 {
                let elapsed = last_log_time.elapsed();
                let blocks_processed = end_block - last_logged_block;
                let blocks_per_sec = blocks_processed as f64 / elapsed.as_secs_f64();
                let progress = (end_block as f64 / head as f64) * 100.0;

                tracing::info!(
                    "Indexed up to block {} / {} ({:.2}%) | {} blocks in {:.2}s ({:.1} blocks/sec)",
                    end_block, head, progress, blocks_processed, elapsed.as_secs_f64(), blocks_per_sec
                );

                last_logged_block = end_block;
                last_log_time = std::time::Instant::now();
            }
        }
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

    async fn truncate_tables(&self) -> Result<()> {
        sqlx::query(
            "TRUNCATE blocks, transactions, addresses, nft_contracts, nft_tokens, nft_transfers,
             erc20_contracts, erc20_transfers, erc20_balances, event_logs, proxy_contracts, indexer_state CASCADE"
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn index_block(&self, provider: &HttpProvider, block_num: u64) -> Result<()> {
        // Fetch block with transactions
        let block = provider
            .get_block_by_number(BlockNumberOrTag::Number(block_num), BlockTransactionsKind::Full)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Block {} not found", block_num))?;

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

        // Fetch receipts and process logs
        let receipts = provider.get_block_receipts(BlockId::Number(BlockNumberOrTag::Number(block_num))).await?;
        if let Some(receipts) = receipts {
            for receipt in receipts {
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

        sqlx::query(
            "INSERT INTO transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, $10)
             ON CONFLICT (hash) DO NOTHING"
        )
        .bind(format!("{:?}", tx_hash))
        .bind(block_number as i64)
        .bind(block_index)
        .bind(format!("{:?}", from_addr))
        .bind(to_addr.map(|a| format!("{:?}", a)))
        .bind(value_decimal)
        .bind(gas_price)
        .bind(gas_limit as i64)
        .bind(input.to_vec())
        .bind(timestamp as i64)
        .execute(&mut **tx)
        .await?;

        // Upsert addresses
        self.upsert_address(tx, from_addr, block_number, false).await?;
        if let Some(to) = to_addr {
            // Check if it's a contract by looking at input data length
            let is_contract = !input.is_empty();
            self.upsert_address(tx, to, block_number, is_contract).await?;
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
             ON CONFLICT (tx_hash, log_index) DO NOTHING"
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
        let exists: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM erc20_contracts WHERE LOWER(address) = LOWER($1)"
        )
        .bind(&contract_address)
        .fetch_optional(&mut **tx)
        .await?;

        if exists.is_none() {
            // Fetch ERC-20 metadata from contract
            let (name, symbol, decimals) = self.fetch_erc20_metadata(provider, log.address()).await;

            sqlx::query(
                "INSERT INTO erc20_contracts (address, name, symbol, decimals, first_seen_block)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (address) DO NOTHING"
            )
            .bind(&contract_address)
            .bind(name)
            .bind(symbol)
            .bind(decimals)
            .bind(block_number as i64)
            .execute(&mut **tx)
            .await?;
        }

        // Insert transfer record
        sqlx::query(
            "INSERT INTO erc20_transfers (tx_hash, log_index, contract_address, from_address, to_address, value, block_number, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (tx_hash, log_index) DO NOTHING"
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

        Ok(())
    }

    /// Fetch ERC-20 metadata (name, symbol, decimals) from contract
    async fn fetch_erc20_metadata(
        &self,
        provider: &HttpProvider,
        address: Address,
    ) -> (Option<String>, Option<String>, i16) {
        // Function selectors
        const NAME_SELECTOR: [u8; 4] = [0x06, 0xfd, 0xde, 0x03]; // name()
        const SYMBOL_SELECTOR: [u8; 4] = [0x95, 0xd8, 0x9b, 0x41]; // symbol()
        const DECIMALS_SELECTOR: [u8; 4] = [0x31, 0x3c, 0xe5, 0x67]; // decimals()

        let name = self.call_string_method(provider, address, &NAME_SELECTOR).await;
        let symbol = self.call_string_method(provider, address, &SYMBOL_SELECTOR).await;
        let decimals = self.call_uint8_method(provider, address, &DECIMALS_SELECTOR).await.unwrap_or(18);

        (name, symbol, decimals as i16)
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
}
