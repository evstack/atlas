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

use crate::api::handlers::get_latest_block;
use crate::api::AppState;
use crate::head::HeadTracker;
use crate::indexer::DaSseUpdate;
use crate::metrics::{Metrics, SseConnectionGuard};
use atlas_common::Block;
use sqlx::PgPool;
use tracing::warn;

#[derive(Serialize, Debug)]
struct NewBlockEvent {
    block: Block,
}

#[derive(Serialize, Debug)]
struct DaUpdateEvent {
    block_number: i64,
    header_da_height: i64,
    data_da_height: i64,
}

#[derive(Serialize, Debug)]
struct DaBatchEvent {
    updates: Vec<DaUpdateEvent>,
}

#[derive(Serialize, Debug)]
struct DaResyncEvent {
    required: bool,
}

/// Build the SSE stream. Separated from the handler for testability.
fn make_event_stream(
    pool: PgPool,
    head_tracker: Arc<HeadTracker>,
    mut block_rx: broadcast::Receiver<()>,
    mut da_rx: broadcast::Receiver<Vec<DaSseUpdate>>,
    metrics: Option<Metrics>,
) -> impl Stream<Item = Result<Event, Infallible>> + Send {
    async_stream::stream! {
        // Guard decrements the SSE connection gauge when the stream is dropped
        let _guard = metrics.map(SseConnectionGuard::new);
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

        loop {
            tokio::select! {
                result = block_rx.recv() => {
                    match result {
                        Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
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
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                result = da_rx.recv() => {
                    match result {
                        Ok(updates) => {
                            if let Some(event) = da_batch_to_event(&updates) {
                                yield Ok(event);
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!(
                                skipped,
                                "sse da: client fell behind DA update stream; requesting resync"
                            );
                            if let Some(event) = da_resync_event() {
                                yield Ok(event);
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    }
}

/// GET /api/events — Server-Sent Events stream for live committed block updates.
/// New connections receive the current latest block and then stream forward from
/// in-memory committed head state, plus DA status update batches. If the DA
/// stream lags, the handler emits `da_resync` so the frontend can refetch the
/// visible DA state instead of silently going stale.
pub async fn block_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = make_event_stream(
        state.pool.clone(),
        state.head_tracker.clone(),
        state.block_events_tx.subscribe(),
        state.da_events_tx.subscribe(),
        Some(state.metrics.clone()),
    );
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

fn da_batch_to_event(updates: &[DaSseUpdate]) -> Option<Event> {
    if updates.is_empty() {
        return None;
    }
    let batch = DaBatchEvent {
        updates: updates
            .iter()
            .map(|da| DaUpdateEvent {
                block_number: da.block_number,
                header_da_height: da.header_da_height,
                data_da_height: da.data_da_height,
            })
            .collect(),
    };
    serde_json::to_string(&batch)
        .ok()
        .map(|json| Event::default().event("da_batch").data(json))
}

fn da_resync_event() -> Option<Event> {
    serde_json::to_string(&DaResyncEvent { required: true })
        .ok()
        .map(|json| Event::default().event("da_resync").data(json))
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

    fn sample_da_update(block_number: i64) -> DaSseUpdate {
        DaSseUpdate {
            block_number,
            header_da_height: block_number * 10,
            data_da_height: block_number * 10 + 1,
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

    #[test]
    fn da_resync_event_serializes_with_required_flag() {
        let event = da_resync_event().expect("event should serialize");
        let debug = format!("{event:?}");
        assert!(debug.contains("da_resync"));
    }

    #[tokio::test]
    async fn stream_seeds_from_head_tracker() {
        let tracker = Arc::new(HeadTracker::empty(10));
        tracker
            .publish_committed_batch(vec![sample_block(42)])
            .await;

        let (tx, _) = broadcast::channel::<()>(16);
        let (da_tx, _) = broadcast::channel::<Vec<DaSseUpdate>>(16);
        let stream = make_event_stream(
            dummy_pool(),
            tracker,
            tx.subscribe(),
            da_tx.subscribe(),
            None,
        );
        tokio::pin!(stream);

        // Drop sender so loop terminates after the initial seed.
        drop(tx);
        drop(da_tx);

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
        let (da_tx, _) = broadcast::channel::<Vec<DaSseUpdate>>(16);
        let stream = make_event_stream(
            dummy_pool(),
            tracker.clone(),
            tx.subscribe(),
            da_tx.subscribe(),
            None,
        );
        tokio::pin!(stream);

        // Consume initial seed.
        let _ = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap();

        // Publish a new block and broadcast.
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
        drop(da_tx);
    }

    #[tokio::test]
    async fn stream_terminates_when_client_behind_tail() {
        // Buffer capacity 3: only keeps 3 most recent blocks.
        let tracker = Arc::new(HeadTracker::empty(3));
        tracker
            .publish_committed_batch(vec![sample_block(10), sample_block(11), sample_block(12)])
            .await;

        let (tx, _) = broadcast::channel::<()>(16);
        let (da_tx, _) = broadcast::channel::<Vec<DaSseUpdate>>(16);
        let stream = make_event_stream(
            dummy_pool(),
            tracker.clone(),
            tx.subscribe(),
            da_tx.subscribe(),
            None,
        );
        tokio::pin!(stream);

        // Consume initial seed (latest = block 12).
        let _ = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap();

        // Advance buffer far ahead: client cursor=12, buffer will be [23,24,25].
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

        // Stream should detect behind-tail and terminate.
        let result = tokio::time::timeout(Duration::from_secs(2), async {
            while (stream.next().await).is_some() {}
        })
        .await;
        assert!(
            result.is_ok(),
            "stream should terminate when client falls behind replay tail"
        );

        drop(tx);
        drop(da_tx);
    }

    #[tokio::test]
    async fn stream_emits_da_resync_when_da_updates_lag() {
        let tracker = Arc::new(HeadTracker::empty(10));
        tracker
            .publish_committed_batch(vec![sample_block(42)])
            .await;

        let (tx, _) = broadcast::channel::<()>(16);
        let (da_tx, _) = broadcast::channel::<Vec<DaSseUpdate>>(1);
        let stream = make_event_stream(
            dummy_pool(),
            tracker,
            tx.subscribe(),
            da_tx.subscribe(),
            None,
        );
        tokio::pin!(stream);

        let _ = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap();

        da_tx.send(vec![sample_da_update(100)]).unwrap();
        da_tx.send(vec![sample_da_update(101)]).unwrap();

        let next = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let debug = format!("{next:?}");
        assert!(debug.contains("da_resync"));

        drop(tx);
        drop(da_tx);
    }
}
