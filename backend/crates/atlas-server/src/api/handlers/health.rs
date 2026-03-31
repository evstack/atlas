use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::api::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// GET /health/live — liveness probe (process is alive)
pub async fn liveness() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok",
        reason: None,
    })
}

/// GET /health/ready — readiness probe (DB reachable, indexer fresh)
pub async fn readiness(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Check DB connectivity
    if let Err(e) = sqlx::query("SELECT 1").execute(&state.pool).await {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "not_ready",
                reason: Some(format!("database unreachable: {e}")),
            }),
        );
    }

    // Check indexer freshness (head within 5 minutes)
    if let Some(block) = state.head_tracker.latest().await {
        let now = chrono::Utc::now();
        let age = now - block.indexed_at;
        if age > chrono::Duration::minutes(5) {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "not_ready",
                    reason: Some(format!(
                        "indexer stale: last block indexed {}s ago",
                        age.num_seconds()
                    )),
                }),
            );
        }
    }

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ready",
            reason: None,
        }),
    )
}
