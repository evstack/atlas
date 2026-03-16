use axum::{
    extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::Stream;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

use crate::api::AppState;
use atlas_common::Block;
use sqlx::PgPool;
use tracing::warn;

const BLOCK_COLUMNS: &str =
    "number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at";
const FETCH_BATCH_SIZE: i64 = 256;

#[derive(Serialize, Debug)]
struct NewBlockEvent {
    block: Block,
}

/// GET /api/events — Server-Sent Events stream for live block updates.
/// Seeds from the latest indexed block, then streams live in-process blocks.
/// If a subscriber lags and drops broadcast messages, it backfills from the DB.
pub async fn block_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let pool = state.pool.clone();
    let mut rx = state.block_events_tx.subscribe();

    let stream = async_stream::stream! {
        let mut last_block_number: Option<i64> = None;

        match fetch_latest_block(&pool).await {
            Ok(Some(block)) => {
                let block_number = block.number;
                last_block_number = Some(block_number);
                if let Some(event) = block_to_event(block) {
                    yield Ok(event);
                }
            }
            Ok(None) => {}
            Err(e) => warn!(error = ?e, "sse: failed to fetch initial block"),
        }

        loop {
            match rx.recv().await {
                Ok(block) => {
                    if last_block_number.is_some_and(|last| block.number <= last) {
                        continue;
                    }

                    last_block_number = Some(block.number);
                    if let Some(event) = block_to_event(block) {
                        yield Ok(event);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(skipped, cursor = ?last_block_number, "sse: lagged on live block stream, backfilling from db");
                    let mut cursor = last_block_number;

                    loop {
                        match fetch_blocks_after(&pool, cursor).await {
                            Ok(blocks) => {
                                if blocks.is_empty() {
                                    break;
                                }

                                let batch_len = blocks.len();
                                for block in blocks {
                                    let block_number = block.number;
                                    last_block_number = Some(block_number);
                                    cursor = Some(block_number);
                                    if let Some(event) = block_to_event(block) {
                                        yield Ok(event);
                                    }
                                }

                                if batch_len < FETCH_BATCH_SIZE as usize {
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!(error = ?e, cursor = ?last_block_number, "sse: failed to backfill blocks after lag");
                                break;
                            }
                        }
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

async fn fetch_latest_block(pool: &PgPool) -> Result<Option<Block>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT {} FROM blocks ORDER BY number DESC LIMIT 1",
        BLOCK_COLUMNS
    ))
    .fetch_optional(pool)
    .await
}

async fn fetch_blocks_after(pool: &PgPool, cursor: Option<i64>) -> Result<Vec<Block>, sqlx::Error> {
    let lower_bound = cursor.unwrap_or(-1);

    sqlx::query_as(&format!(
        "SELECT {} FROM blocks WHERE number > $1 ORDER BY number ASC LIMIT {}",
        BLOCK_COLUMNS, FETCH_BATCH_SIZE
    ))
    .bind(lower_bound)
    .fetch_all(pool)
    .await
}

fn block_to_event(block: Block) -> Option<Event> {
    let block_id = block.number.to_string();
    let event = NewBlockEvent { block };
    serde_json::to_string(&event)
        .ok()
        .map(|json| Event::default().event("new_block").id(block_id).data(json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_block(number: i64) -> Block {
        Block {
            number,
            hash: format!("0x{:064x}", number),
            parent_hash: format!("0x{:064x}", number.saturating_sub(1)),
            timestamp: 1_700_000_000 + number,
            gas_used: 21_000,
            gas_limit: 30_000_000,
            transaction_count: 1,
            indexed_at: Utc::now(),
        }
    }

    #[test]
    fn new_block_event_serializes_with_block_wrapper() {
        let event = NewBlockEvent {
            block: sample_block(42),
        };
        let json = serde_json::to_string(&event).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(
            v.get("block").is_some(),
            "event JSON must contain a 'block' key"
        );
        assert_eq!(v["block"]["number"], 42);
        assert_eq!(v["block"]["gas_used"], 21_000);
        assert_eq!(v["block"]["transaction_count"], 1);
    }

    #[test]
    fn new_block_event_contains_all_block_fields() {
        let event = NewBlockEvent {
            block: sample_block(1),
        };
        let json = serde_json::to_string(&event).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let block = &v["block"];

        for field in [
            "number",
            "hash",
            "parent_hash",
            "timestamp",
            "gas_used",
            "gas_limit",
            "transaction_count",
            "indexed_at",
        ] {
            assert!(
                block.get(field).is_some(),
                "block JSON missing field: {field}"
            );
        }
    }
}
