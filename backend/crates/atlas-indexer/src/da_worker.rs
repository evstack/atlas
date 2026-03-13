//! Background DA (Data Availability) worker for tracking Celestia inclusion status.
//!
//! This worker queries ev-node's Connect RPC service to determine at which Celestia
//! height each block's header and data were submitted.
//!
//! ## Two-phase design
//!
//! The worker runs in a loop with two phases:
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
//!    Processes oldest pending blocks first.
//!
//! A block flows: backfill (phase 1) → update-pending (phase 2) → done.

use anyhow::Result;
use futures::stream::{self, StreamExt};
use sqlx::PgPool;
use std::time::Duration;

use crate::evnode::EvnodeClient;

/// Batch size for DB queries per cycle.
const BATCH_SIZE: i64 = 100;

/// Sleep between worker cycles.
const CYCLE_SLEEP: Duration = Duration::from_secs(2);

pub struct DaWorker {
    pool: PgPool,
    client: EvnodeClient,
    concurrency: usize,
}

impl DaWorker {
    pub fn new(pool: PgPool, evnode_url: &str, concurrency: u32) -> Result<Self> {
        Ok(Self {
            pool,
            client: EvnodeClient::new(evnode_url),
            concurrency: concurrency as usize,
        })
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!(
            "DA worker started (concurrency: {})",
            self.concurrency
        );

        loop {
            // Phase 1: discover and check new blocks (newest first)
            let backfilled = self.backfill_new_blocks().await?;

            // Phase 2: retry blocks pending DA inclusion (oldest first)
            let updated = self.update_pending_blocks().await?;

            if backfilled > 0 || updated > 0 {
                tracing::info!(
                    "DA worker cycle: backfilled {}, updated {} pending",
                    backfilled,
                    updated
                );
            }

            tokio::time::sleep(CYCLE_SLEEP).await;
        }
    }

    /// Phase 1: Find blocks missing from block_da_status and query ev-node.
    /// Returns the number of blocks processed.
    async fn backfill_new_blocks(&self) -> Result<usize> {
        let missing: Vec<(i64,)> = sqlx::query_as(
            "SELECT b.number FROM blocks b
             LEFT JOIN block_da_status d ON d.block_number = b.number
             WHERE d.block_number IS NULL
             ORDER BY b.number DESC
             LIMIT $1",
        )
        .bind(BATCH_SIZE)
        .fetch_all(&self.pool)
        .await?;

        if missing.is_empty() {
            return Ok(0);
        }

        let count = missing.len();
        let pool = &self.pool;
        let client = &self.client;

        stream::iter(missing)
            .map(|(block_number,)| async move {
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
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to fetch DA status for block {}: {}",
                            block_number,
                            e
                        );
                    }
                }
            })
            .buffer_unordered(self.concurrency)
            .collect::<Vec<()>>()
            .await;

        Ok(count)
    }

    /// Phase 2: Re-check blocks where DA heights are still 0.
    /// Returns the number of blocks processed.
    async fn update_pending_blocks(&self) -> Result<usize> {
        let pending: Vec<(i64,)> = sqlx::query_as(
            "SELECT block_number FROM block_da_status
             WHERE header_da_height = 0 OR data_da_height = 0
             ORDER BY block_number ASC
             LIMIT $1",
        )
        .bind(BATCH_SIZE)
        .fetch_all(&self.pool)
        .await?;

        if pending.is_empty() {
            return Ok(0);
        }

        let count = pending.len();
        let pool = &self.pool;
        let client = &self.client;

        stream::iter(pending)
            .map(|(block_number,)| async move {
                match client.get_da_status(block_number as u64).await {
                    Ok((header_da, data_da)) => {
                        if let Err(e) = sqlx::query(
                            "UPDATE block_da_status
                             SET header_da_height = $2, data_da_height = $3, updated_at = NOW()
                             WHERE block_number = $1",
                        )
                        .bind(block_number)
                        .bind(header_da as i64)
                        .bind(data_da as i64)
                        .execute(pool)
                        .await
                        {
                            tracing::warn!(
                                "Failed to update DA status for block {}: {}",
                                block_number,
                                e
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to fetch DA status for block {}: {}",
                            block_number,
                            e
                        );
                    }
                }
            })
            .buffer_unordered(self.concurrency)
            .collect::<Vec<()>>()
            .await;

        Ok(count)
    }
}
