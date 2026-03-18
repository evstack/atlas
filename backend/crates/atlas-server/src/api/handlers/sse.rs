use axum::{
    extract::State,
    response::IntoResponse,
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
use crate::head::HeadTracker;
use atlas_common::Block;
use sqlx::PgPool;
use tracing::warn;

type SseStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

#[derive(Serialize, Debug)]
struct NewBlockEvent {
    block: Block,
}

/// Build the SSE block stream. Separated from the handler for testability.
fn make_block_stream(
    pool: PgPool,
    head_tracker: Arc<HeadTracker>,
    mut rx: broadcast::Receiver<()>,
) -> impl Stream<Item = Result<Event, Infallible>> + Send {
    async_stream::stream! {
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
            let snapshot = head_tracker.replay_after(last_block_number).await;

            if let (Some(cursor), Some(buffer_start)) = (last_block_number, snapshot.buffer_start) {
                if cursor + 1 < buffer_start {
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
    }
}

/// GET /api/events — Server-Sent Events stream for live committed block updates.
/// New connections receive only the current latest block and then stream
/// forward from in-memory committed head state. Historical catch-up stays on
/// the canonical block endpoints.
pub async fn block_events(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let stream = make_block_stream(
        state.pool.clone(),
        state.head_tracker.clone(),
        state.block_events_tx.subscribe(),
    );
    sse_response(stream)
}

fn sse_response<S>(stream: S) -> impl IntoResponse
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
    use crate::head::HeadTracker;
    use chrono::Utc;
    use futures::StreamExt;

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

    /// Lazy PgPool that never connects — safe for tests that don't hit the DB.
    fn dummy_pool() -> PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost:5432/test")
            .expect("lazy pool creation should not fail")
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

    #[tokio::test]
    async fn stream_seeds_from_head_tracker() {
        let tracker = Arc::new(HeadTracker::empty(10));
        tracker
            .publish_committed_batch(vec![sample_block(42)])
            .await;

        let (tx, _) = broadcast::channel::<()>(16);
        let rx = tx.subscribe();
        let stream = make_block_stream(dummy_pool(), tracker, rx);
        tokio::pin!(stream);

        // Drop sender so loop terminates after the initial seed
        drop(tx);

        let first = tokio::time::timeout(Duration::from_secs(1), stream.next()).await;
        assert!(
            first.is_ok(),
            "stream should yield initial event without blocking"
        );
        assert!(
            first.unwrap().is_some(),
            "stream should yield at least one event"
        );
    }

    #[tokio::test]
    async fn stream_replays_new_blocks_after_broadcast() {
        let tracker = Arc::new(HeadTracker::empty(10));
        tracker
            .publish_committed_batch(vec![sample_block(42)])
            .await;

        let (tx, _) = broadcast::channel::<()>(16);
        let rx = tx.subscribe();
        let stream = make_block_stream(dummy_pool(), tracker.clone(), rx);
        tokio::pin!(stream);

        // Consume initial seed
        let _ = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap();

        // Publish a new block and broadcast
        tracker
            .publish_committed_batch(vec![sample_block(43)])
            .await;
        tx.send(()).unwrap();

        let second = tokio::time::timeout(Duration::from_secs(1), stream.next()).await;
        assert!(second.is_ok(), "stream should yield event after broadcast");
        assert!(
            second.unwrap().is_some(),
            "broadcast should trigger a new event"
        );

        drop(tx);
    }

    #[tokio::test]
    async fn stream_terminates_when_client_behind_tail() {
        // Buffer capacity 3: only keeps 3 most recent blocks
        let tracker = Arc::new(HeadTracker::empty(3));
        tracker
            .publish_committed_batch(vec![sample_block(10), sample_block(11), sample_block(12)])
            .await;

        let (tx, _) = broadcast::channel::<()>(16);
        let rx = tx.subscribe();
        let stream = make_block_stream(dummy_pool(), tracker.clone(), rx);
        tokio::pin!(stream);

        // Consume initial seed (latest = block 12)
        let _ = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap();

        // Advance buffer far ahead: client cursor=12, buffer will be [23,24,25]
        tracker
            .publish_committed_batch(vec![
                sample_block(20),
                sample_block(21),
                sample_block(22),
                sample_block(23),
                sample_block(24),
                sample_block(25),
            ])
            .await;
        tx.send(()).unwrap();

        // Stream should detect behind-tail and terminate
        let result = tokio::time::timeout(Duration::from_secs(2), async {
            while (stream.next().await).is_some() {}
        })
        .await;
        assert!(
            result.is_ok(),
            "stream should terminate when client falls behind replay tail"
        );
    }
}
