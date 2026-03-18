use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::api::error::ApiResult;
use crate::api::AppState;

#[derive(Serialize)]
pub struct ChainFeatures {
    pub da_tracking: bool,
}

#[derive(Serialize)]
pub struct ChainStatus {
    pub block_height: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_at: Option<String>,
    pub features: ChainFeatures,
}

/// GET /api/status - Lightweight endpoint for current chain status
/// Returns in <1ms, optimized for frequent polling
pub async fn get_status(State(state): State<Arc<AppState>>) -> ApiResult<Json<ChainStatus>> {
    let features = ChainFeatures {
        da_tracking: state.da_tracking_enabled,
    };

    if let Some(block) = state.head_tracker.latest().await {
        return Ok(Json(ChainStatus {
            block_height: block.number,
            indexed_at: Some(block.indexed_at.to_rfc3339()),
            features,
        }));
    }

    // Fallback: single key-value lookup from indexer_state (sub-ms, avoids blocks table)
    let row: Option<(i64, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT value::bigint, updated_at FROM indexer_state WHERE key = 'last_indexed_block'",
    )
    .fetch_optional(&state.pool)
    .await?;

    if let Some((block_height, updated_at)) = row {
        return Ok(Json(ChainStatus {
            block_height,
            indexed_at: Some(updated_at.to_rfc3339()),
            features,
        }));
    }

    Ok(Json(ChainStatus {
        block_height: 0,
        indexed_at: None,
        features,
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
        let (block_tx, _) = tokio::sync::broadcast::channel(1);
        let (da_tx, _) = tokio::sync::broadcast::channel(1);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost:5432/test")
            .expect("lazy pool");
        State(Arc::new(AppState {
            pool,
            block_events_tx: block_tx,
            da_events_tx: da_tx,
            head_tracker,
            rpc_url: String::new(),
            da_tracking_enabled: false,
        }))
    }

    #[tokio::test]
    async fn status_returns_head_tracker_block() {
        let tracker = Arc::new(HeadTracker::empty(10));
        tracker
            .publish_committed_batch(vec![sample_block(42)])
            .await;

        let result = get_status(test_state(tracker)).await;
        let Json(status) = result.unwrap_or_else(|_| panic!("get_status should not fail"));

        assert_eq!(status.block_height, 42);
        assert!(status.indexed_at.is_some());
        assert!(!status.features.da_tracking);
    }

    #[tokio::test]
    async fn status_returns_latest_head_after_multiple_publishes() {
        let tracker = Arc::new(HeadTracker::empty(10));
        tracker
            .publish_committed_batch(vec![sample_block(10), sample_block(11), sample_block(12)])
            .await;

        let result = get_status(test_state(tracker)).await;
        let Json(status) = result.unwrap_or_else(|_| panic!("get_status should not fail"));

        assert_eq!(status.block_height, 12);
    }
}
