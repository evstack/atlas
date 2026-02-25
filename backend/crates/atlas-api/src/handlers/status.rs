use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::error::ApiResult;
use crate::handlers::get_table_count;
use crate::AppState;

#[derive(Serialize)]
pub struct HeightResponse {
    pub block_height: i64,
    pub indexed_at: String,
}

#[derive(Serialize)]
pub struct ChainStatus {
    pub chain_id: u64,
    pub chain_name: String,
    pub block_height: i64,
    pub total_transactions: i64,
    pub total_addresses: i64,
    pub indexed_at: String,
}

/// GET /api/height - Lightweight endpoint for current block height.
/// Returns in <1ms, optimized for frequent polling.
pub async fn get_height(State(state): State<Arc<AppState>>) -> ApiResult<Json<HeightResponse>> {
    let result: (String, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "SELECT value, updated_at FROM indexer_state WHERE key = 'last_indexed_block'",
    )
    .fetch_one(&state.pool)
    .await?;

    let block_height: i64 = result.0.parse().unwrap_or(0);

    Ok(Json(HeightResponse {
        block_height,
        indexed_at: result.1.to_rfc3339(),
    }))
}

/// GET /api/status - Full chain status including chain ID, name, and counts.
pub async fn get_status(State(state): State<Arc<AppState>>) -> ApiResult<Json<ChainStatus>> {
    let result: (String, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "SELECT value, updated_at FROM indexer_state WHERE key = 'last_indexed_block'",
    )
    .fetch_one(&state.pool)
    .await?;

    let block_height: i64 = result.0.parse().unwrap_or(0);

    let total_transactions = get_table_count(&state.pool, "transactions").await?;
    let total_addresses = get_table_count(&state.pool, "addresses").await?;

    Ok(Json(ChainStatus {
        chain_id: state.chain_id,
        chain_name: state.chain_name.clone(),
        block_height,
        total_transactions,
        total_addresses,
        indexed_at: result.1.to_rfc3339(),
    }))
}
