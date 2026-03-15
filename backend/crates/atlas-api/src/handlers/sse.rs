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
use tokio::time::sleep;

use crate::AppState;
use atlas_common::{Block, BlockDaStatus};
use sqlx::{postgres::PgListener, PgPool};
use tracing::warn;

const BLOCK_COLUMNS: &str =
    "number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at";
const BLOCK_EVENT_CHANNEL: &str = "atlas_new_blocks";
const DA_EVENT_CHANNEL: &str = "atlas_da_updates";
const FETCH_BATCH_SIZE: i64 = 256;

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

/// GET /api/events — Server-Sent Events stream for live block and DA updates.
/// Seeds from the latest indexed block, then requeries the DB for blocks added
/// after that point whenever the shared notification fanout emits a wake-up.
/// Also streams DA status updates when the DA worker processes blocks.
pub async fn block_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let pool = state.pool.clone();
    let mut block_rx = state.block_events_tx.subscribe();
    let mut da_rx = state.da_events_tx.subscribe();

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
            tokio::select! {
                result = block_rx.recv() => {
                    match result {
                        Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
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
                                        warn!(error = ?e, cursor = ?last_block_number, "sse: failed to fetch blocks after wake-up");
                                        break;
                                    }
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                result = da_rx.recv() => {
                    match result {
                        Ok(block_numbers) => {
                            match fetch_da_status(&pool, &block_numbers).await {
                                Ok(da_rows) => {
                                    if let Some(event) = da_batch_to_event(&da_rows) {
                                        yield Ok(event);
                                    }
                                }
                                Err(e) => {
                                    warn!(error = ?e, "sse: failed to fetch DA status for update");
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // Missed some DA updates — frontend will catch up on next poll/update
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

pub async fn run_block_event_fanout(
    database_url: String,
    _pool: PgPool,
    tx: broadcast::Sender<()>,
) {
    loop {
        let mut listener = match PgListener::connect(&database_url).await {
            Ok(listener) => listener,
            Err(e) => {
                warn!(error = ?e, "sse: failed to connect Postgres listener");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        if let Err(e) = listener.listen(BLOCK_EVENT_CHANNEL).await {
            warn!(error = ?e, channel = BLOCK_EVENT_CHANNEL, "sse: failed to LISTEN for block notifications");
            sleep(Duration::from_secs(1)).await;
            continue;
        }

        // Wake all subscribers once after reconnect so they can requery the DB.
        let _ = tx.send(());

        loop {
            match listener.recv().await {
                Ok(_) => {
                    let _ = tx.send(());
                }
                Err(e) => {
                    warn!(error = ?e, "sse: Postgres listener disconnected");
                    break;
                }
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

pub async fn run_da_event_fanout(
    database_url: String,
    tx: broadcast::Sender<Vec<i64>>,
) {
    loop {
        let mut listener = match PgListener::connect(&database_url).await {
            Ok(listener) => listener,
            Err(e) => {
                warn!(error = ?e, "sse: failed to connect DA Postgres listener");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        if let Err(e) = listener.listen(DA_EVENT_CHANNEL).await {
            warn!(error = ?e, channel = DA_EVENT_CHANNEL, "sse: failed to LISTEN for DA notifications");
            sleep(Duration::from_secs(1)).await;
            continue;
        }

        loop {
            match listener.recv().await {
                Ok(notification) => {
                    if let Ok(block_numbers) =
                        serde_json::from_str::<Vec<i64>>(notification.payload())
                    {
                        let _ = tx.send(block_numbers);
                    }
                }
                Err(e) => {
                    warn!(error = ?e, "sse: DA Postgres listener disconnected");
                    break;
                }
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
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

async fn fetch_da_status(
    pool: &PgPool,
    block_numbers: &[i64],
) -> Result<Vec<BlockDaStatus>, sqlx::Error> {
    sqlx::query_as(
        "SELECT block_number, header_da_height, data_da_height, updated_at
         FROM block_da_status
         WHERE block_number = ANY($1)",
    )
    .bind(block_numbers)
    .fetch_all(pool)
    .await
}

fn block_to_event(block: Block) -> Option<Event> {
    let event = NewBlockEvent { block };
    serde_json::to_string(&event)
        .ok()
        .map(|json| Event::default().event("new_block").data(json))
}

fn da_batch_to_event(rows: &[BlockDaStatus]) -> Option<Event> {
    if rows.is_empty() {
        return None;
    }
    let batch = DaBatchEvent {
        updates: rows
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