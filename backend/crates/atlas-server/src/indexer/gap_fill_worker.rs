//! Background worker that retries blocks stored in the `failed_blocks` table.
//!
//! ## Design
//!
//! The main indexer writes blocks that fail after 3 inline retries to the
//! `failed_blocks` table. This worker polls that table and re-fetches those
//! blocks using exponential backoff based on `retry_count`.
//!
//! Each cycle the worker queries for blocks whose backoff window has elapsed,
//! fetches them one at a time via RPC, writes them using the same COPY path
//! as the main indexer, and removes them from `failed_blocks` on success.
//! On failure the `retry_count` is incremented and the block is left for a
//! future cycle.

use anyhow::Result;
use governor::{Quota, RateLimiter};
use sqlx::PgPool;
use std::collections::HashSet;
use std::num::NonZeroU32;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

use super::batch::BlockBatch;
use super::fetcher::{fetch_blocks_batch, FetchResult, SharedRateLimiter};
use super::indexer::{ensure_partitions_exist, Indexer};
use crate::metrics::Metrics;

/// Maximum blocks processed per cycle.
const BATCH_SIZE: i64 = 50;

/// Sleep when there is no work ready to retry.
const IDLE_SLEEP: Duration = Duration::from_secs(300);

/// Atomically claim a batch of blocks whose backoff window has elapsed.
/// Backoff: 2 * 2^min(retry_count, 10) minutes.
/// At retry_count=3 (minimum after inline retries): ~16 min.
/// At retry_count=10+: ~34 h (capped).
///
/// The CTE uses `FOR UPDATE SKIP LOCKED` so concurrent workers never claim
/// the same row. Setting `last_failed_at = NOW()` on the claimed rows means
/// they fall outside the backoff window for any other worker that runs before
/// this cycle finishes.
const SELECT_READY_SQL: &str = "
    WITH candidates AS (
        SELECT block_number FROM failed_blocks
        WHERE last_failed_at < NOW() - make_interval(mins => (2 * power(2, LEAST(retry_count, 10)))::int)
        ORDER BY block_number DESC
        LIMIT $1
        FOR UPDATE SKIP LOCKED
    )
    UPDATE failed_blocks SET last_failed_at = NOW()
    FROM candidates
    WHERE failed_blocks.block_number = candidates.block_number
    RETURNING failed_blocks.block_number";

pub struct GapFillWorker {
    pool: PgPool,
    database_url: String,
    rpc_url: String,
    rpc_requests_per_second: u32,
    block_events_tx: broadcast::Sender<()>,
    metrics: Metrics,
    current_max_partition: AtomicU64,
}

impl GapFillWorker {
    pub fn new(
        pool: PgPool,
        database_url: &str,
        rpc_url: &str,
        rpc_requests_per_second: u32,
        block_events_tx: broadcast::Sender<()>,
        metrics: Metrics,
    ) -> Result<Self> {
        if rpc_requests_per_second == 0 {
            anyhow::bail!("rpc_requests_per_second must be greater than 0");
        }
        Ok(Self {
            pool,
            database_url: database_url.to_string(),
            rpc_url: rpc_url.to_string(),
            rpc_requests_per_second,
            block_events_tx,
            metrics,
            current_max_partition: AtomicU64::new(0),
        })
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!("Gap-fill worker started");
        loop {
            let (attempted, succeeded) = self.process_batch().await?;
            if attempted > 0 {
                tracing::info!(attempted, succeeded, "gap-fill worker cycle complete");
            } else {
                tokio::time::sleep(IDLE_SLEEP).await;
            }
        }
    }

    /// Fetch one batch of eligible failed blocks, retry them, and return
    /// `(attempted, succeeded)`.
    pub async fn process_batch(&self) -> Result<(usize, usize)> {
        let blocks: Vec<(i64,)> = sqlx::query_as(SELECT_READY_SQL)
            .bind(BATCH_SIZE)
            .fetch_all(&self.pool)
            .await?;

        if blocks.is_empty() {
            return Ok((0, 0));
        }

        let rps = NonZeroU32::new(self.rpc_requests_per_second).unwrap();
        let rate_limiter: SharedRateLimiter = Arc::new(RateLimiter::direct(Quota::per_second(rps)));
        let http_client = reqwest::Client::new();
        // Empty sets: re-discovered contracts are re-inserted with ON CONFLICT DO NOTHING.
        let known_erc20: HashSet<String> = HashSet::new();
        let known_nft: HashSet<String> = HashSet::new();

        let mut copy_client = Indexer::connect_copy_client(&self.database_url).await?;

        let attempted = blocks.len();
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        for (block_number,) in blocks {
            let block_num = block_number as u64;
            let results = fetch_blocks_batch(
                &http_client,
                &self.rpc_url,
                block_num,
                1,
                &rate_limiter,
                &self.metrics,
            )
            .await;

            match results.into_iter().next() {
                Some(FetchResult::Success(fetched)) => {
                    let mut batch = BlockBatch::new();
                    Indexer::collect_block(&mut batch, &known_erc20, &known_nft, *fetched);

                    if let Err(e) =
                        ensure_partitions_exist(&self.pool, &self.current_max_partition, block_num)
                            .await
                    {
                        tracing::warn!(block = block_num, error = %e, "gap-fill: partition check failed");
                        self.increment_retry(block_number).await?;
                        failed += 1;
                        continue;
                    }

                    if let Err(e) = Indexer::write_batch_and_clear_failed_block(
                        &mut copy_client,
                        batch,
                        block_number,
                    )
                    .await
                    {
                        tracing::warn!(block = block_num, error = %e, "gap-fill: write failed");
                        self.increment_retry(block_number).await?;
                        failed += 1;
                        continue;
                    }

                    let _ = self.block_events_tx.send(());
                    tracing::info!(block = block_num, "gap-fill: block recovered");
                    succeeded += 1;
                }
                Some(FetchResult::Error { error, .. }) => {
                    tracing::warn!(block = block_num, error, "gap-fill: fetch failed");
                    self.increment_retry(block_number).await?;
                    failed += 1;
                }
                None => {
                    tracing::warn!(block = block_num, "gap-fill: fetch returned no result");
                    self.increment_retry(block_number).await?;
                    failed += 1;
                }
            }
        }

        if failed > 0 {
            tracing::warn!(succeeded, failed, "gap-fill cycle done with failures");
        }

        if succeeded > 0 {
            self.metrics
                .set_indexer_missing_blocks(self.get_missing_block_count().await?);
        }

        Ok((attempted, succeeded))
    }

    async fn get_missing_block_count(&self) -> Result<u64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM failed_blocks")
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0.max(0) as u64)
    }

    async fn increment_retry(&self, block_number: i64) -> Result<()> {
        let result = sqlx::query(
            "UPDATE failed_blocks SET retry_count = retry_count + 1, last_failed_at = NOW()
             WHERE block_number = $1",
        )
        .bind(block_number)
        .execute(&self.pool)
        .await?;

        anyhow::ensure!(
            result.rows_affected() == 1,
            "gap-fill: increment_retry affected {} rows for block {}",
            result.rows_affected(),
            block_number
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pool() -> PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost:5432/test")
            .expect("lazy pool creation should not fail")
    }

    #[tokio::test]
    async fn new_rejects_zero_rps() {
        let (tx, _) = broadcast::channel(1);
        let err = GapFillWorker::new(
            test_pool(),
            "postgres://test@localhost:5432/test",
            "http://localhost:8545",
            0,
            tx,
            Metrics::new(),
        )
        .err()
        .expect("zero rps should fail");

        assert!(err
            .to_string()
            .contains("rpc_requests_per_second must be greater than 0"));
    }

    #[test]
    fn select_ready_sql_has_backoff() {
        assert!(SELECT_READY_SQL.contains("power(2, LEAST(retry_count"));
    }

    #[test]
    fn select_ready_sql_orders_desc() {
        assert!(SELECT_READY_SQL.contains("ORDER BY block_number DESC"));
    }

    #[test]
    fn select_ready_sql_uses_skip_locked() {
        assert!(SELECT_READY_SQL.contains("FOR UPDATE SKIP LOCKED"));
    }
}
