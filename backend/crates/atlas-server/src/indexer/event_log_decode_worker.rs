use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::time::Duration;

use atlas_common::EventLog;

use crate::event_log_decode::{
    apply_decode_attempt, build_decoder_for_address, EVENT_LOG_DECODE_PENDING,
};

const JOB_BATCH_SIZE: i64 = 25;
const LOG_BATCH_SIZE: i64 = 500;
const IDLE_SLEEP: Duration = Duration::from_secs(5);

const CLAIM_JOBS_SQL: &str = "
    WITH candidates AS (
        SELECT address
        FROM event_log_decode_jobs
        ORDER BY full_rescan DESC, requested_at ASC
        LIMIT $1
        FOR UPDATE SKIP LOCKED
    )
    UPDATE event_log_decode_jobs AS jobs
    SET last_attempted_at = NOW(), updated_at = NOW()
    FROM candidates
    WHERE jobs.address = candidates.address
    RETURNING jobs.address, jobs.full_rescan, jobs.updated_at";

pub struct EventLogDecodeWorker {
    pool: PgPool,
    rpc_url: String,
}

impl EventLogDecodeWorker {
    pub fn new(pool: PgPool, rpc_url: &str) -> Self {
        Self {
            pool,
            rpc_url: rpc_url.to_string(),
        }
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!("Event log decode worker started");

        loop {
            let processed = self.process_batch().await?;
            if processed == 0 {
                tokio::time::sleep(IDLE_SLEEP).await;
            }
        }
    }

    async fn process_batch(&self) -> Result<usize> {
        let jobs: Vec<(String, bool, DateTime<Utc>)> = sqlx::query_as(CLAIM_JOBS_SQL)
            .bind(JOB_BATCH_SIZE)
            .fetch_all(&self.pool)
            .await?;

        if jobs.is_empty() {
            return Ok(0);
        }

        for (address, full_rescan, claim_token) in &jobs {
            match self.process_job(address, *full_rescan).await {
                Ok(()) => {
                    sqlx::query(
                        "DELETE FROM event_log_decode_jobs
                         WHERE address = $1 AND updated_at = $2",
                    )
                    .bind(address)
                    .bind(*claim_token)
                    .execute(&self.pool)
                    .await?;
                }
                Err(error) => {
                    tracing::warn!(
                        address = %address,
                        full_rescan,
                        error = %error,
                        "event log decode job failed"
                    );
                    sqlx::query(
                        "UPDATE event_log_decode_jobs
                         SET retry_count = retry_count + 1, error_message = $3
                         WHERE address = $1 AND updated_at = $2",
                    )
                    .bind(address)
                    .bind(*claim_token)
                    .bind(error.to_string())
                    .execute(&self.pool)
                    .await?;
                }
            }
        }

        Ok(jobs.len())
    }

    async fn process_job(&self, address: &str, full_rescan: bool) -> Result<()> {
        let decoder = build_decoder_for_address(&self.pool, &self.rpc_url, address).await?;
        let mut cursor: Option<(i64, i32, String)> = None;

        loop {
            let logs = self
                .fetch_logs(address, full_rescan, cursor.as_ref())
                .await?;
            if logs.is_empty() {
                break;
            }

            let attempted_at = Utc::now();
            let mut tx = self.pool.begin().await?;
            for log in &logs {
                let attempt = match &decoder {
                    Some(decoder) => decoder.decode_log(log),
                    None => crate::event_log_decode::DecodeAttempt {
                        decoded: None,
                        decode_status: crate::event_log_decode::EVENT_LOG_DECODE_NO_ABI,
                        decode_source: None,
                    },
                };
                let outcome = apply_decode_attempt(log, attempt, attempted_at);

                sqlx::query(
                    "UPDATE event_logs
                     SET decoded = $1,
                         decode_status = $2,
                         decode_source = $3,
                         decoded_at = $4,
                         decode_attempted_at = $5
                     WHERE id = $6 AND block_number = $7",
                )
                .bind(&outcome.decoded)
                .bind(&outcome.decode_status)
                .bind(&outcome.decode_source)
                .bind(outcome.decoded_at)
                .bind(outcome.decode_attempted_at)
                .bind(log.id)
                .bind(log.block_number)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;

            let last = logs.last().expect("non-empty logs batch");
            cursor = Some((last.block_number, last.log_index, last.tx_hash.clone()));
        }

        Ok(())
    }

    async fn fetch_logs(
        &self,
        address: &str,
        full_rescan: bool,
        cursor: Option<&(i64, i32, String)>,
    ) -> Result<Vec<EventLog>> {
        let rows = match (full_rescan, cursor) {
            (true, Some((block_number, log_index, tx_hash))) => {
                sqlx::query_as(
                    "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3,
                            data, block_number, decoded, decode_status, decoded_at,
                            decode_attempted_at, decode_source
                     FROM event_logs
                     WHERE address = $1
                       AND (block_number, log_index, tx_hash) > ($2, $3, $4)
                     ORDER BY block_number ASC, log_index ASC, tx_hash ASC
                     LIMIT $5",
                )
                .bind(address)
                .bind(*block_number)
                .bind(*log_index)
                .bind(tx_hash)
                .bind(LOG_BATCH_SIZE)
                .fetch_all(&self.pool)
                .await?
            }
            (true, None) => {
                sqlx::query_as(
                    "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3,
                            data, block_number, decoded, decode_status, decoded_at,
                            decode_attempted_at, decode_source
                     FROM event_logs
                     WHERE address = $1
                     ORDER BY block_number ASC, log_index ASC, tx_hash ASC
                     LIMIT $2",
                )
                .bind(address)
                .bind(LOG_BATCH_SIZE)
                .fetch_all(&self.pool)
                .await?
            }
            (false, Some((block_number, log_index, tx_hash))) => {
                sqlx::query_as(
                    "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3,
                            data, block_number, decoded, decode_status, decoded_at,
                            decode_attempted_at, decode_source
                     FROM event_logs
                     WHERE address = $1
                       AND decode_status = $2
                       AND (block_number, log_index, tx_hash) > ($3, $4, $5)
                     ORDER BY block_number ASC, log_index ASC, tx_hash ASC
                     LIMIT $6",
                )
                .bind(address)
                .bind(EVENT_LOG_DECODE_PENDING)
                .bind(*block_number)
                .bind(*log_index)
                .bind(tx_hash)
                .bind(LOG_BATCH_SIZE)
                .fetch_all(&self.pool)
                .await?
            }
            (false, None) => {
                sqlx::query_as(
                    "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3,
                            data, block_number, decoded, decode_status, decoded_at,
                            decode_attempted_at, decode_source
                     FROM event_logs
                     WHERE address = $1 AND decode_status = $2
                     ORDER BY block_number ASC, log_index ASC, tx_hash ASC
                     LIMIT $3",
                )
                .bind(address)
                .bind(EVENT_LOG_DECODE_PENDING)
                .bind(LOG_BATCH_SIZE)
                .fetch_all(&self.pool)
                .await?
            }
        };

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_jobs_sql_uses_skip_locked() {
        assert!(CLAIM_JOBS_SQL.contains("FOR UPDATE SKIP LOCKED"));
    }

    #[test]
    fn incremental_mode_looks_for_pending_logs() {
        assert_eq!(EVENT_LOG_DECODE_PENDING, "pending");
    }
}
