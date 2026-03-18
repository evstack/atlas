//! Background DA (Data Availability) worker for tracking Celestia inclusion status.
//!
//! This worker queries ev-node's Connect RPC service to determine at which Celestia
//! height each block's header and data were submitted.
//!
//! ## Two-phase design
//!
//! The worker runs in a loop with a fixed RPC budget per cycle (BATCH_SIZE):
//!
//! 1. **Backfill** — Discovers blocks in the `blocks` table that are missing from
//!    `block_da_status`. Queries ev-node for each and INSERTs the result.
//!    **Always inserts a row, even when DA heights are 0** (block not yet included
//!    on Celestia). This marks the block as "checked" so the backfill phase won't
//!    re-query it on the next cycle. Processes newest blocks first so the UI shows
//!    current data immediately.
//!
//! 2. **Update pending** — Finds rows where `header_da_height = 0 OR data_da_height = 0`
//!    and re-queries ev-node. Updates with new values when the block has been included.
//!    Processes newest pending blocks first (most relevant to UI users).
//!
//! Both phases share the same per-cycle RPC budget. Backfill runs first and takes
//! what it needs; pending gets the remainder. This ensures new blocks are checked
//! promptly while pending blocks still make progress every cycle.
//!
//! A block flows: backfill (phase 1) → update-pending (phase 2) → done.
//!
//! After each batch, the worker sends updated block numbers through an in-process
//! broadcast channel so the SSE handler can push live DA status changes to clients.

use anyhow::Result;
use futures::stream::{self, StreamExt};
use governor::{Quota, RateLimiter};
use sqlx::PgPool;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

use super::evnode::EvnodeClient;

/// Total RPC budget per cycle, split between backfill and pending.
const BATCH_SIZE: i64 = 100;

/// Sleep when idle (no work in either phase).
const IDLE_SLEEP: Duration = Duration::from_millis(500);

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
}

impl DaWorker {
    pub fn new(
        pool: PgPool,
        evnode_url: &str,
        concurrency: u32,
        requests_per_second: u32,
        da_events_tx: broadcast::Sender<Vec<DaSseUpdate>>,
    ) -> Result<Self> {
        let rate = NonZeroU32::new(requests_per_second)
            .ok_or_else(|| anyhow::anyhow!("DA_RPC_REQUESTS_PER_SECOND must be greater than 0"))?;
        Ok(Self {
            pool,
            client: EvnodeClient::new(evnode_url),
            concurrency: concurrency as usize,
            requests_per_second,
            rate_limiter: Arc::new(RateLimiter::direct(Quota::per_second(rate))),
            da_events_tx,
        })
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!(
            "DA worker started (concurrency: {}, rate_limit: {} req/s)",
            self.concurrency,
            self.requests_per_second
        );

        loop {
            // Phase 1: backfill gets first pick of the budget
            let backfilled = self.backfill_new_blocks(BATCH_SIZE).await?;

            // Phase 2: pending gets whatever budget remains
            let remaining = BATCH_SIZE - backfilled as i64;
            let updated = if remaining > 0 {
                self.update_pending_blocks(remaining).await?
            } else {
                0
            };

            let did_work = backfilled > 0 || updated > 0;
            if did_work {
                tracing::info!(
                    "DA worker cycle: backfilled {}, updated {} pending",
                    backfilled,
                    updated
                );
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

    /// Phase 1: Find blocks missing from block_da_status and query ev-node.
    /// Returns the number of blocks processed.
    async fn backfill_new_blocks(&self, limit: i64) -> Result<usize> {
        let missing: Vec<(i64,)> = sqlx::query_as(
            "SELECT b.number FROM blocks b
             LEFT JOIN block_da_status d ON d.block_number = b.number
             WHERE d.block_number IS NULL
             ORDER BY b.number DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        if missing.is_empty() {
            return Ok(0);
        }

        let pool = &self.pool;
        let client = &self.client;
        let rate_limiter = &self.rate_limiter;

        let results: Vec<Option<DaSseUpdate>> = stream::iter(missing)
            .map(|(block_number,)| async move {
                rate_limiter.until_ready().await;
                match client.get_da_status(block_number as u64).await {
                    Ok((header_da, data_da)) => {
                        if let Err(e) = sqlx::query(
                            "INSERT INTO block_da_status (block_number, header_da_height, data_da_height)
                             VALUES ($1, $2, $3)
                             ON CONFLICT (block_number) DO UPDATE SET
                                header_da_height = EXCLUDED.header_da_height,
                                data_da_height = EXCLUDED.data_da_height,
                                updated_at = NOW()",
                        )
                        .bind(block_number)
                        .bind(header_da as i64)
                        .bind(data_da as i64)
                        .execute(pool)
                        .await
                        {
                            tracing::warn!(
                                "Failed to insert DA status for block {}: {}",
                                block_number,
                                e
                            );
                            return None;
                        }
                        Some(DaSseUpdate {
                            block_number,
                            header_da_height: header_da as i64,
                            data_da_height: data_da as i64,
                        })
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to fetch DA status for block {}: {}",
                            block_number,
                            e
                        );
                        None
                    }
                }
            })
            .buffer_unordered(self.concurrency)
            .collect()
            .await;

        let updates: Vec<DaSseUpdate> = results.into_iter().flatten().collect();
        self.notify_da_updates(&updates);

        Ok(updates.len())
    }

    /// Phase 2: Re-check blocks where DA heights are still 0.
    /// Returns the number of blocks processed.
    async fn update_pending_blocks(&self, limit: i64) -> Result<usize> {
        let pending: Vec<(i64,)> = sqlx::query_as(
            "SELECT block_number FROM block_da_status
             WHERE header_da_height = 0 OR data_da_height = 0
             ORDER BY block_number DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        if pending.is_empty() {
            return Ok(0);
        }

        let pool = &self.pool;
        let client = &self.client;
        let rate_limiter = &self.rate_limiter;

        let results: Vec<Option<DaSseUpdate>> = stream::iter(pending)
            .map(|(block_number,)| async move {
                rate_limiter.until_ready().await;
                match client.get_da_status(block_number as u64).await {
                    Ok((header_da, data_da)) => {
                        match sqlx::query(
                            "UPDATE block_da_status
                             SET header_da_height = $2, data_da_height = $3, updated_at = NOW()
                             WHERE block_number = $1
                               AND (header_da_height, data_da_height) IS DISTINCT FROM ($2, $3)",
                        )
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
                                tracing::warn!(
                                    "Failed to update DA status for block {}: {}",
                                    block_number,
                                    e
                                );
                                None
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to fetch DA status for block {}: {}",
                            block_number,
                            e
                        );
                        None
                    }
                }
            })
            .buffer_unordered(self.concurrency)
            .collect()
            .await;

        let updates: Vec<DaSseUpdate> = results.into_iter().flatten().collect();
        self.notify_da_updates(&updates);

        Ok(updates.len())
    }
}
