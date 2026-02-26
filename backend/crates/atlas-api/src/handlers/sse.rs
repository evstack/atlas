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

#[derive(Serialize)]
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
                let latest: Option<i64> = sqlx::query_scalar("SELECT MAX(number) FROM blocks")
                    .fetch_one(&state.pool)
                    .await
                    .ok()
                    .flatten();

                if let Some(max_num) = latest {
                    last_block_number = Some(max_num);
                    // Emit the current latest block as the initial event
                    let block: Option<Block> = sqlx::query_as(
                        "SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at
                         FROM blocks WHERE number = $1"
                    )
                    .bind(max_num)
                    .fetch_optional(&state.pool)
                    .await
                    .ok()
                    .flatten();

                    if let Some(block) = block {
                        let event = NewBlockEvent { block };
                        if let Ok(json) = serde_json::to_string(&event) {
                            yield Ok(Event::default().event("new_block").data(json));
                        }
                    }
                    ping_counter = 0;
                }
                continue;
            }

            let cursor = last_block_number.unwrap();

            // Fetch ALL new blocks since last sent, in ascending order
            let new_blocks: Vec<Block> = sqlx::query_as(
                "SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at
                 FROM blocks WHERE number > $1 ORDER BY number ASC"
            )
            .bind(cursor)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();

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
