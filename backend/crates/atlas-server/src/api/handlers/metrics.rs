use axum::extract::State;
use std::sync::Arc;

use crate::api::AppState;

/// GET /metrics — Prometheus text format
pub async fn metrics(State(state): State<Arc<AppState>>) -> String {
    state.prometheus_handle.render()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::head::HeadTracker;
    use crate::metrics::Metrics;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::OnceLock;
    use tokio::sync::broadcast;

    fn test_prometheus_handle() -> metrics_exporter_prometheus::PrometheusHandle {
        static PROMETHEUS_HANDLE: OnceLock<metrics_exporter_prometheus::PrometheusHandle> =
            OnceLock::new();

        PROMETHEUS_HANDLE
            .get_or_init(crate::metrics::install_prometheus_recorder)
            .clone()
    }

    #[tokio::test]
    async fn metrics_handler_renders_prometheus_output() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost:5432/test")
            .expect("lazy pool");
        let (block_tx, _) = broadcast::channel(1);
        let (da_tx, _) = broadcast::channel(1);
        let prometheus_handle = test_prometheus_handle();
        let recorder_metrics = Metrics::new();
        recorder_metrics.set_indexer_head_block(42);
        let state = Arc::new(AppState {
            pool,
            block_events_tx: block_tx,
            da_events_tx: da_tx,
            head_tracker: Arc::new(HeadTracker::empty(10)),
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
            metrics: recorder_metrics,
            prometheus_handle,
        });

        let body = super::metrics(State(state)).await;

        assert!(body.contains("atlas_indexer_head_block"));
    }
}
