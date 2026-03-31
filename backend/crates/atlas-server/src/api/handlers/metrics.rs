use axum::extract::State;
use std::sync::Arc;

use crate::api::AppState;

/// GET /metrics — Prometheus text format
pub async fn metrics(State(state): State<Arc<AppState>>) -> String {
    state.prometheus_handle.render()
}
