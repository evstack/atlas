use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::AppState;
use crate::error::ApiResult;

#[derive(Serialize)]
pub struct ChainStatus {
    pub block_height: i64,
    pub indexed_at: String,
}

/// GET /api/status - Lightweight endpoint for current chain status
/// Returns in <1ms, optimized for frequent polling
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ChainStatus>> {
    let result: (String, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "SELECT value, updated_at FROM indexer_state WHERE key = 'last_indexed_block'"
    )
    .fetch_one(&state.pool)
    .await?;

    let block_height: i64 = result.0.parse().unwrap_or(0);

    Ok(Json(ChainStatus {
        block_height,
        indexed_at: result.1.to_rfc3339(),
    }))
}
