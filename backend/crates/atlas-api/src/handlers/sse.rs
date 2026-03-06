use axum::{
    extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::Stream;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

use crate::AppState;
use atlas_common::Block;
use tracing::warn;

#[derive(Serialize, Debug)]
struct NewBlockEvent {
    block: Block,
}

/// GET /api/events — Server-Sent Events stream for live block updates.
/// Polls the DB every 200ms and emits one `new_block` event per block, in order.
/// Never skips blocks — fetches all blocks since the last one sent.
pub async fn block_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = async_stream::stream! {
        let mut last_block_number: Option<i64> = None;
        let mut tick = interval(Duration::from_millis(200));
        let mut ping_counter: u32 = 0;

        loop {
            tick.tick().await;
            ping_counter += 1;

            // On first tick, seed with the latest block number
            if last_block_number.is_none() {
                let latest: Option<i64> = match sqlx::query_scalar("SELECT MAX(number) FROM blocks")
                    .fetch_one(&state.pool)
                    .await
                {
                    Ok(v) => v,
                    Err(e) => { warn!(error = ?e, "sse: failed to query latest block number"); continue; }
                };

                if let Some(max_num) = latest {
                    // Emit the current latest block as the initial event.
                    // Only advance the cursor after a successful fetch-and-emit so the
                    // block is not skipped if the fetch fails.
                    let block: Option<Block> = match sqlx::query_as(
                        "SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at
                         FROM blocks WHERE number = $1"
                    )
                    .bind(max_num)
                    .fetch_optional(&state.pool)
                    .await
                    {
                        Ok(v) => v,
                        Err(e) => { warn!(error = ?e, "sse: failed to fetch initial block"); continue; }
                    };

                    if let Some(block) = block {
                        last_block_number = Some(block.number);
                        let event = NewBlockEvent { block };
                        if let Ok(json) = serde_json::to_string(&event) {
                            yield Ok(Event::default().event("new_block").data(json));
                        }
                        ping_counter = 0;
                    }
                }
                continue;
            }

            let cursor = last_block_number.unwrap();

            // Fetch new blocks since last sent, in ascending order (capped to avoid
            // unbounded memory usage and to stay well within the 10s statement_timeout).
            let new_blocks: Vec<Block> = match sqlx::query_as(
                "SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at
                 FROM blocks WHERE number > $1 ORDER BY number ASC LIMIT 100"
            )
            .bind(cursor)
            .fetch_all(&state.pool)
            .await
            {
                Ok(rows) => rows,
                Err(e) => { warn!(error = ?e, cursor, "sse: failed to fetch new blocks"); continue; }
            };

            if !new_blocks.is_empty() {
                ping_counter = 0;
            }

            // Emit one event per block, in order
            for block in new_blocks {
                last_block_number = Some(block.number);

                let event = NewBlockEvent { block };
                if let Ok(json) = serde_json::to_string(&event) {
                    yield Ok(Event::default().event("new_block").data(json));
                }
            }

            // Send keep-alive ping every ~15s (75 ticks * 200ms)
            if ping_counter >= 75 {
                ping_counter = 0;
                yield Ok(Event::default().comment("keep-alive"));
            }
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
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

        assert!(v.get("block").is_some(), "event JSON must contain a 'block' key");
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
