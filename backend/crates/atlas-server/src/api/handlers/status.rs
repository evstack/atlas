use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::api::error::ApiResult;
use crate::api::handlers::get_table_count;
use crate::api::AppState;

#[derive(Serialize)]
pub struct HeightResponse {
    pub block_height: i64,
    pub indexed_at: String,
}

#[derive(Serialize)]
pub struct ChainStatus {
    pub chain_id: String,
    pub chain_name: String,
    pub block_height: i64,
    pub total_transactions: i64,
    pub total_addresses: i64,
    pub indexed_at: String,
}

async fn latest_height_and_indexed_at(state: &AppState) -> Result<(i64, String), sqlx::Error> {
    if let Some(block) = state.head_tracker.latest().await {
        return Ok((block.number, block.indexed_at.to_rfc3339()));
    }

    // Fallback: single key-value lookup from indexer_state (sub-ms, avoids blocks table)
    let row: Option<(i64, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT value::bigint, updated_at FROM indexer_state WHERE key = 'last_indexed_block'",
    )
    .fetch_optional(&state.pool)
    .await?;

    if let Some((block_height, updated_at)) = row {
        return Ok((block_height, updated_at.to_rfc3339()));
    }

    Ok((0, String::new()))
}

/// GET /api/height - Lightweight endpoint for current block height.
/// Returns in <1ms, optimized for frequent polling.
pub async fn get_height(State(state): State<Arc<AppState>>) -> ApiResult<Json<HeightResponse>> {
    let (block_height, indexed_at) = latest_height_and_indexed_at(&state).await?;

    Ok(Json(HeightResponse {
        block_height,
        indexed_at,
    }))
}

/// GET /api/status - Full chain status including chain ID, name, and counts.
pub async fn get_status(State(state): State<Arc<AppState>>) -> ApiResult<Json<ChainStatus>> {
    let (block_height, indexed_at) = latest_height_and_indexed_at(&state).await?;
    let total_transactions = get_table_count(&state.pool, "transactions").await?;
    let total_addresses = get_table_count(&state.pool, "addresses").await?;

    Ok(Json(ChainStatus {
        chain_id: state.chain_id.to_string(),
        chain_name: state.chain_name.clone(),
        block_height,
        total_transactions,
        total_addresses,
        indexed_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::head::HeadTracker;
    use atlas_common::Block;
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

    fn test_state(head_tracker: Arc<HeadTracker>) -> State<Arc<AppState>> {
        let (tx, _) = tokio::sync::broadcast::channel(1);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost:5432/test")
            .expect("lazy pool");
        State(Arc::new(AppState {
            pool,
            block_events_tx: tx,
            head_tracker,
            rpc_url: String::new(),
            faucet: None,
            chain_id: 1,
            chain_name: "Test Chain".to_string(),
        }))
    }

    #[tokio::test]
    async fn height_returns_head_tracker_block() {
        let tracker = Arc::new(HeadTracker::empty(10));
        tracker
            .publish_committed_batch(vec![sample_block(42)])
            .await;

        let result = get_height(test_state(tracker)).await;
        let Json(status) = result.unwrap_or_else(|_| panic!("get_height should not fail"));

        assert_eq!(status.block_height, 42);
        assert!(!status.indexed_at.is_empty());
    }

    #[tokio::test]
    async fn height_returns_latest_head_after_multiple_publishes() {
        let tracker = Arc::new(HeadTracker::empty(10));
        tracker
            .publish_committed_batch(vec![sample_block(10), sample_block(11), sample_block(12)])
            .await;

        let result = get_height(test_state(tracker)).await;
        let Json(status) = result.unwrap_or_else(|_| panic!("get_height should not fail"));

        assert_eq!(status.block_height, 12);
    }
}
