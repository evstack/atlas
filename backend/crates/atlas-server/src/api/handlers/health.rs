use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;

use crate::api::AppState;

const MAX_INDEXER_AGE_MINUTES: i64 = 5;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn readiness_status(
    latest_indexed_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> (StatusCode, HealthResponse) {
    let Some(indexed_at) = latest_indexed_at else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            HealthResponse {
                status: "not_ready",
                reason: Some("indexer state unavailable".to_string()),
            },
        );
    };

    let age = now - indexed_at;
    if age > chrono::Duration::minutes(MAX_INDEXER_AGE_MINUTES) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            HealthResponse {
                status: "not_ready",
                reason: Some(format!(
                    "indexer stale: last block indexed {}s ago",
                    age.num_seconds()
                )),
            },
        );
    }

    (
        StatusCode::OK,
        HealthResponse {
            status: "ready",
            reason: None,
        },
    )
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
        tracing::warn!(error = %e, "readiness database check failed");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "not_ready",
                reason: Some("database unreachable".to_string()),
            }),
        );
    }

    let latest = match super::status::latest_indexed_block(state.as_ref()).await {
        Ok(latest) => latest,
        Err(e) => {
            tracing::warn!(error = %e, "readiness indexer state check failed");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "not_ready",
                    reason: Some("indexer state unavailable".to_string()),
                }),
            );
        }
    };

    let (status, body) = readiness_status(latest.map(|(_, indexed_at)| indexed_at), Utc::now());
    (status, Json(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::head::HeadTracker;
    use crate::metrics::Metrics;
    use axum::body::to_bytes;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    fn app_state(pool: sqlx::PgPool, head_tracker: Arc<HeadTracker>) -> Arc<AppState> {
        let (block_tx, _) = broadcast::channel(1);
        let (da_tx, _) = broadcast::channel(1);
        let prometheus_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
            .build_recorder()
            .handle();

        Arc::new(AppState {
            pool,
            block_events_tx: block_tx,
            da_events_tx: da_tx,
            head_tracker,
            rpc_url: String::new(),
            da_tracking_enabled: false,
            faucet: None,
            chain_id: 1,
            chain_name: "Test Chain".to_string(),
            chain_logo_url: None,
            chain_logo_url_light: None,
            chain_logo_url_dark: None,
            accent_color: None,
            background_color_dark: None,
            background_color_light: None,
            success_color: None,
            error_color: None,
            metrics: Metrics::new(),
            prometheus_handle,
            solc_cache_dir: "/tmp/solc-cache".to_string(),
        })
    }

    async fn json_response(response: axum::response::Response) -> (StatusCode, serde_json::Value) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let json = serde_json::from_slice(&body).expect("parse json response");
        (status, json)
    }

    #[tokio::test]
    async fn liveness_returns_ok() {
        let (status, json) = json_response(liveness().await.into_response()).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "ok");
        assert!(json.get("reason").is_none());
    }

    #[tokio::test]
    async fn readiness_returns_unavailable_when_database_is_down() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@127.0.0.1:1/atlas")
            .expect("create lazy pool");
        let state = app_state(pool, Arc::new(HeadTracker::empty(10)));

        let (status, json) = json_response(readiness(State(state)).await.into_response()).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(json["status"], "not_ready");
        assert_eq!(json["reason"], "database unreachable");
    }

    #[test]
    fn readiness_returns_unavailable_when_indexer_state_is_missing() {
        let (status, body) = readiness_status(None, Utc::now());
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert_eq!(body.reason.as_deref(), Some("indexer state unavailable"));
    }

    #[test]
    fn readiness_returns_unavailable_for_stale_indexer_state() {
        let (status, body) = readiness_status(
            Some(Utc::now() - chrono::Duration::minutes(MAX_INDEXER_AGE_MINUTES + 1)),
            Utc::now(),
        );
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert!(body
            .reason
            .as_deref()
            .expect("reason string")
            .contains("indexer stale"));
    }

    #[test]
    fn readiness_returns_ready_for_fresh_indexer_state() {
        let (status, body) = readiness_status(Some(Utc::now()), Utc::now());
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.status, "ready");
        assert!(body.reason.is_none());
    }
}
