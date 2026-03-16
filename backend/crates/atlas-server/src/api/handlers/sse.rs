use axum::{
    extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::Stream;
use serde::Serialize;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

use crate::api::handlers::get_latest_block;
use crate::api::AppState;
use atlas_common::Block;
use tracing::warn;

type SseStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

#[derive(Serialize, Debug)]
struct NewBlockEvent {
    block: Block,
}

/// GET /api/events — Server-Sent Events stream for live committed block updates.
/// New connections receive only the current latest block and then stream
/// forward from in-memory committed head state. Historical catch-up stays on
/// the canonical block endpoints.
pub async fn block_events(State(state): State<Arc<AppState>>) -> Sse<SseStream> {
    let pool = state.pool.clone();
    let head_tracker = state.head_tracker.clone();
    let mut rx = state.block_events_tx.subscribe();

    let stream = async_stream::stream! {
        let mut last_block_number: Option<i64> = None;

        match head_tracker.latest().await {
            Some(block) => {
                last_block_number = Some(block.number);
                if let Some(event) = block_to_event(block) {
                    yield Ok(event);
                }
            }
            None => match get_latest_block(&pool).await {
                Ok(Some(block)) => {
                    last_block_number = Some(block.number);
                    if let Some(event) = block_to_event(block) {
                        yield Ok(event);
                    }
                }
                Ok(None) => {}
                Err(e) => warn!(error = ?e, "sse: failed to fetch initial block"),
            },
        }

        while let Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) = rx.recv().await {
            if let Some(cursor) = last_block_number {
                let snapshot = head_tracker.replay_after(Some(cursor)).await;

                if let Some(buffer_start) = snapshot.buffer_start {
                    if cursor < buffer_start.saturating_sub(1) {
                        warn!(
                            last_seen = cursor,
                            buffer_start,
                            buffer_end = ?snapshot.buffer_end,
                            "sse head-only: client fell behind replay tail; closing stream for canonical refetch"
                        );
                        break;
                    }
                }

                if !snapshot.blocks_after_cursor.is_empty() {
                    for block in snapshot.blocks_after_cursor {
                        last_block_number = Some(block.number);
                        if let Some(event) = block_to_event(block) {
                            yield Ok(event);
                        }
                    }
                    continue;
                }
            }

            match head_tracker.latest().await {
                Some(block) if last_block_number.is_none_or(|last_seen| block.number > last_seen) => {
                    last_block_number = Some(block.number);
                    if let Some(event) = block_to_event(block) {
                        yield Ok(event);
                    }
                }
                Some(_) | None => {}
            }
        }
    };

    sse_response(stream)
}

fn sse_response<S>(stream: S) -> Sse<SseStream>
where
    S: Stream<Item = Result<Event, Infallible>> + Send + 'static,
{
    let stream: SseStream = Box::pin(stream);

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

fn block_to_event(block: Block) -> Option<Event> {
    let event = NewBlockEvent { block };
    serde_json::to_string(&event)
        .ok()
        .map(|json| Event::default().event("new_block").data(json))
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
