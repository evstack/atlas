use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::api::error::ApiResult;
use crate::api::handlers::get_latest_block;
use crate::api::AppState;

#[derive(Serialize)]
pub struct ChainStatus {
    pub block_height: i64,
    pub indexed_at: String,
}

/// GET /api/status - Lightweight endpoint for current chain status
/// Returns in <1ms, optimized for frequent polling
pub async fn get_status(State(state): State<Arc<AppState>>) -> ApiResult<Json<ChainStatus>> {
    if let Some(block) = state.head_tracker.latest().await {
        return Ok(Json(ChainStatus {
            block_height: block.number,
            indexed_at: block.indexed_at.to_rfc3339(),
        }));
    }

    if let Some(block) = get_latest_block(&state.pool).await? {
        return Ok(Json(ChainStatus {
            block_height: block.number,
            indexed_at: block.indexed_at.to_rfc3339(),
        }));
    }

    Ok(Json(ChainStatus {
        block_height: 0,
        indexed_at: String::new(),
    }))
}
