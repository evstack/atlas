//! Background DA (Data Availability) worker for tracking Celestia inclusion status.
//!
//! This worker queries ev-node's Connect RPC service to determine at which Celestia
//! height each block's header and data were submitted.
//!
//! ## Design
//!
//! Each cycle the worker fetches the BATCH_SIZE highest-numbered blocks that still
//! need DA info — either because they have no entry in `block_da_status` yet, or
//! because a previous check returned 0 heights (not yet included on Celestia).
//! Both cases are handled by a single unified query ordered by block number DESC,
//! so fresh blocks always get priority over the historical backfill.
//!
//! After each batch, updated block numbers are sent through an in-process broadcast
//! channel so the SSE handler can push live DA status changes to clients.

use anyhow::Result;
use futures::stream::{self, StreamExt};
use governor::{Quota, RateLimiter};
use sqlx::PgPool;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

use super::evnode::EvnodeClient;
use crate::metrics::Metrics;

/// Maximum blocks processed per cycle.
const BATCH_SIZE: i64 = 100;

/// Sleep when there is no work to do.
const IDLE_SLEEP: Duration = Duration::from_millis(500);

/// Unified query: blocks missing from block_da_status OR still at 0 heights,
/// newest first so fresh blocks are always prioritized over historical backfill.
const SELECT_NEEDS_DA_SQL: &str = "
    SELECT b.number FROM blocks b
    LEFT JOIN block_da_status d ON d.block_number = b.number
    WHERE d.block_number IS NULL
       OR d.header_da_height = 0
       OR d.data_da_height = 0
    ORDER BY b.number DESC
    LIMIT $1";

const UPSERT_DA_STATUS_SQL: &str = "
    INSERT INTO block_da_status (block_number, header_da_height, data_da_height)
    VALUES ($1, $2, $3)
    ON CONFLICT (block_number) DO UPDATE SET
        header_da_height = EXCLUDED.header_da_height,
        data_da_height   = EXCLUDED.data_da_height,
        updated_at       = NOW()
    WHERE (block_da_status.header_da_height, block_da_status.data_da_height)
              IS DISTINCT FROM (EXCLUDED.header_da_height, EXCLUDED.data_da_height)";

#[derive(Clone, Debug)]
pub struct DaSseUpdate {
    pub block_number: i64,
    pub header_da_height: i64,
    pub data_da_height: i64,
}

pub struct DaWorker {
    pool: PgPool,
    client: EvnodeClient,
    concurrency: usize,
    requests_per_second: u32,
    rate_limiter: Arc<
        RateLimiter<
            governor::state::NotKeyed,
            governor::state::InMemoryState,
            governor::clock::DefaultClock,
        >,
    >,
    da_events_tx: broadcast::Sender<Vec<DaSseUpdate>>,
    metrics: Metrics,
}

impl DaWorker {
    pub fn new(
        pool: PgPool,
        evnode_url: &str,
        concurrency: u32,
        requests_per_second: u32,
        da_events_tx: broadcast::Sender<Vec<DaSseUpdate>>,
        metrics: Metrics,
    ) -> Result<Self> {
        let concurrency = NonZeroU32::new(concurrency)
            .ok_or_else(|| anyhow::anyhow!("DA_WORKER_CONCURRENCY must be greater than 0"))?;
        let rate = NonZeroU32::new(requests_per_second)
            .ok_or_else(|| anyhow::anyhow!("DA_RPC_REQUESTS_PER_SECOND must be greater than 0"))?;
        Ok(Self {
            pool,
            client: EvnodeClient::new(evnode_url),
            concurrency: concurrency.get() as usize,
            requests_per_second,
            rate_limiter: Arc::new(RateLimiter::direct(Quota::per_second(rate))),
            da_events_tx,
            metrics,
        })
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!(
            concurrency = self.concurrency,
            rate_limit_rps = self.requests_per_second,
            "DA worker started"
        );

        loop {
            let processed = self.process_blocks(BATCH_SIZE).await?;
            if processed > 0 {
                self.metrics.record_da_blocks_processed(processed as u64);
                tracing::info!(processed, "DA worker cycle complete");
            } else {
                tokio::time::sleep(IDLE_SLEEP).await;
            }
        }
    }

    /// Notify SSE subscribers of DA status changes via in-process broadcast channel.
    fn notify_da_updates(&self, updates: &[DaSseUpdate]) {
        if updates.is_empty() {
            return;
        }
        let _ = self.da_events_tx.send(updates.to_vec());
    }

    /// Fetch DA status for the highest-numbered blocks that still need it.
    /// Returns the number of blocks where DA heights actually changed.
    async fn process_blocks(&self, limit: i64) -> Result<usize> {
        let blocks: Vec<(i64,)> = sqlx::query_as(SELECT_NEEDS_DA_SQL)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        if blocks.is_empty() {
            return Ok(0);
        }

        let pool = &self.pool;
        let client = &self.client;
        let rate_limiter = &self.rate_limiter;
        let metrics = &self.metrics;

        let mut total_updated = 0usize;

        stream::iter(blocks)
            .map(|(block_number,)| async move {
                rate_limiter.until_ready().await;
                match client.get_da_status(block_number as u64).await {
                    Ok((header_da, data_da)) => {
                        match sqlx::query(UPSERT_DA_STATUS_SQL)
                            .bind(block_number)
                            .bind(header_da as i64)
                            .bind(data_da as i64)
                            .execute(pool)
                            .await
                        {
                            Ok(result) if result.rows_affected() > 0 => Some(DaSseUpdate {
                                block_number,
                                header_da_height: header_da as i64,
                                data_da_height: data_da as i64,
                            }),
                            Ok(_) => None,
                            Err(e) => {
                                metrics.error("da_worker", "da_upsert");
                                tracing::warn!(
                                    block = block_number,
                                    error = %e,
                                    "failed to upsert DA status"
                                );
                                None
                            }
                        }
                    }
                    Err(e) => {
                        metrics.record_da_rpc_error();
                        metrics.error("da_worker", "da_fetch");
                        tracing::warn!(
                            block = block_number,
                            error = %e,
                            "failed to fetch DA status"
                        );
                        None
                    }
                }
            })
            .buffer_unordered(self.concurrency)
            .for_each(|result| {
                if let Some(update) = result {
                    total_updated += 1;
                    self.notify_da_updates(std::slice::from_ref(&update));
                }
                std::future::ready(())
            })
            .await;

        Ok(total_updated)
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
    async fn new_rejects_zero_concurrency() {
        let (tx, _) = broadcast::channel(1);
        let err = DaWorker::new(
            test_pool(),
            "http://localhost:7331",
            0,
            50,
            tx,
            Metrics::new(),
        )
        .err()
        .expect("zero concurrency should fail");

        assert!(err
            .to_string()
            .contains("DA_WORKER_CONCURRENCY must be greater than 0"));
    }

    #[tokio::test]
    async fn new_rejects_zero_rate_limit() {
        let (tx, _) = broadcast::channel(1);
        let err = DaWorker::new(
            test_pool(),
            "http://localhost:7331",
            4,
            0,
            tx,
            Metrics::new(),
        )
        .err()
        .expect("zero rate limit should fail");

        assert!(err
            .to_string()
            .contains("DA_RPC_REQUESTS_PER_SECOND must be greater than 0"));
    }

    #[tokio::test]
    async fn notify_da_updates_sends_full_batch() {
        let (tx, mut rx) = broadcast::channel(1);
        let worker = DaWorker::new(
            test_pool(),
            "http://localhost:7331",
            4,
            50,
            tx,
            Metrics::new(),
        )
        .unwrap();
        let updates = vec![
            DaSseUpdate {
                block_number: 10,
                header_da_height: 100,
                data_da_height: 101,
            },
            DaSseUpdate {
                block_number: 11,
                header_da_height: 110,
                data_da_height: 111,
            },
        ];

        worker.notify_da_updates(&updates);

        let received = rx.recv().await.expect("batch should be broadcast");
        assert_eq!(received.len(), 2);
        assert_eq!(received[0].block_number, 10);
        assert_eq!(received[1].data_da_height, 111);
    }

    #[tokio::test]
    async fn notify_da_updates_skips_empty_batch() {
        let (tx, mut rx) = broadcast::channel(1);
        let worker = DaWorker::new(
            test_pool(),
            "http://localhost:7331",
            4,
            50,
            tx,
            Metrics::new(),
        )
        .unwrap();

        worker.notify_da_updates(&[]);

        let result = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await;
        assert!(result.is_err(), "empty batch should not be broadcast");
    }

    #[test]
    fn query_prioritizes_newest_blocks() {
        assert!(SELECT_NEEDS_DA_SQL.contains("ORDER BY b.number DESC"));
        assert!(SELECT_NEEDS_DA_SQL.contains("LIMIT $1"));
        // Covers both missing rows and 0-height rows in one pass
        assert!(SELECT_NEEDS_DA_SQL.contains("d.block_number IS NULL"));
        assert!(SELECT_NEEDS_DA_SQL.contains("header_da_height = 0"));
        assert!(SELECT_NEEDS_DA_SQL.contains("data_da_height = 0"));
    }

    #[test]
    fn upsert_suppresses_noop_writes() {
        assert!(UPSERT_DA_STATUS_SQL.contains("IS DISTINCT FROM"));
    }
}
