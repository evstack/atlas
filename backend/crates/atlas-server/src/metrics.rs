use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};
use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusHandle};
use std::sync::OnceLock;
use std::time::Instant;

/// Install the Prometheus recorder and return a handle for rendering metrics.
pub fn install_prometheus_recorder() -> PrometheusHandle {
    static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

    PROMETHEUS_HANDLE
        .get_or_init(|| {
            metrics_exporter_prometheus::PrometheusBuilder::new()
                .set_buckets_for_metric(
                    Matcher::Full("atlas_indexer_block_processing_duration_seconds".to_string()),
                    &[
                        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
                    ],
                )
                .expect("valid processing duration buckets")
                .install_recorder()
                .expect("failed to install Prometheus recorder")
        })
        .clone()
}

/// Central metrics registry.
///
/// All metric handles are resolved once at startup after the Prometheus recorder
/// is installed. The struct is `Clone` (metric handles are internally `Arc`).
#[derive(Clone)]
pub struct Metrics {
    _private: (), // force construction via Metrics::new()
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    /// Create all metric handles and register their descriptions.
    /// Must be called after `install_prometheus_recorder()`.
    pub fn new() -> Self {
        // -- HTTP --
        describe_counter!(
            "atlas_http_requests_total",
            "Total HTTP requests by method, path, and status"
        );
        describe_histogram!(
            "atlas_http_request_duration_seconds",
            "HTTP request latency in seconds"
        );

        // -- Indexer --
        describe_counter!(
            "atlas_indexer_blocks_indexed_total",
            "Total blocks successfully indexed"
        );
        describe_gauge!("atlas_indexer_head_block", "Latest indexed block number");
        describe_gauge!(
            "atlas_indexer_head_block_timestamp_seconds",
            "Chain timestamp of the latest indexed block"
        );
        describe_gauge!(
            "atlas_indexer_chain_head_block",
            "Latest block on chain (from RPC)"
        );
        describe_gauge!(
            "atlas_indexer_lag_blocks",
            "Difference between chain head and latest indexed block"
        );
        describe_gauge!(
            "atlas_indexer_missing_blocks",
            "Known unresolved missing blocks persisted in failed_blocks"
        );
        describe_histogram!(
            "atlas_indexer_batch_duration_seconds",
            "Time per full indexer batch cycle"
        );
        describe_histogram!(
            "atlas_indexer_block_processing_duration_seconds",
            "Time spent actively processing a batch, excluding idle sleep"
        );
        describe_histogram!(
            "atlas_indexer_db_write_duration_seconds",
            "Time for DB COPY+INSERT per batch"
        );
        describe_counter!(
            "atlas_indexer_failed_blocks_total",
            "Blocks that permanently failed after retries"
        );
        describe_counter!(
            "atlas_indexer_rpc_requests_total",
            "RPC batch requests by status"
        );

        // -- DA Worker --
        describe_counter!(
            "atlas_da_blocks_processed_total",
            "DA status checks completed"
        );
        describe_counter!("atlas_da_rpc_errors_total", "Failed DA RPC calls");

        // -- Metadata Fetcher --
        describe_counter!(
            "atlas_metadata_contracts_fetched_total",
            "Contract metadata successfully fetched by type"
        );
        describe_counter!(
            "atlas_metadata_tokens_fetched_total",
            "NFT token metadata successfully fetched"
        );
        describe_counter!(
            "atlas_metadata_errors_total",
            "Failed metadata fetches by type"
        );

        // -- SSE --
        describe_gauge!(
            "atlas_sse_active_connections",
            "Current number of SSE client connections"
        );

        // -- DB Pools --
        describe_gauge!("atlas_db_pool_size", "Total connections in pool");
        describe_gauge!("atlas_db_pool_idle", "Idle connections in pool");
        describe_gauge!("atlas_db_pool_max", "Max configured connections for pool");

        // -- Errors --
        describe_counter!(
            "atlas_errors_total",
            "All errors by component and error_type, for alerting"
        );

        Self { _private: () }
    }

    // -- Indexer helpers --

    pub fn record_blocks_indexed(&self, count: u64) {
        counter!("atlas_indexer_blocks_indexed_total").increment(count);
    }

    pub fn set_indexer_head_block(&self, block: u64) {
        gauge!("atlas_indexer_head_block").set(block as f64);
    }

    pub fn set_indexer_head_block_timestamp(&self, timestamp_seconds: i64) {
        gauge!("atlas_indexer_head_block_timestamp_seconds").set(timestamp_seconds as f64);
    }

    pub fn set_chain_head_block(&self, block: u64) {
        gauge!("atlas_indexer_chain_head_block").set(block as f64);
    }

    pub fn set_indexer_lag_blocks(&self, lag: u64) {
        gauge!("atlas_indexer_lag_blocks").set(lag as f64);
    }

    pub fn set_indexer_missing_blocks(&self, count: u64) {
        gauge!("atlas_indexer_missing_blocks").set(count as f64);
    }

    pub fn record_batch_duration(&self, seconds: f64) {
        histogram!("atlas_indexer_batch_duration_seconds").record(seconds);
    }

    pub fn record_block_processing_duration(&self, seconds: f64) {
        histogram!("atlas_indexer_block_processing_duration_seconds").record(seconds);
    }

    pub fn record_db_write_duration(&self, seconds: f64) {
        histogram!("atlas_indexer_db_write_duration_seconds").record(seconds);
    }

    pub fn record_failed_blocks(&self, count: u64) {
        counter!("atlas_indexer_failed_blocks_total").increment(count);
    }

    pub fn record_rpc_request(&self, status: &str) {
        counter!("atlas_indexer_rpc_requests_total", "status" => status.to_string()).increment(1);
    }

    // -- DA Worker helpers --

    pub fn record_da_blocks_processed(&self, count: u64) {
        counter!("atlas_da_blocks_processed_total").increment(count);
    }

    pub fn record_da_rpc_error(&self) {
        counter!("atlas_da_rpc_errors_total").increment(1);
    }

    // -- Metadata Fetcher helpers --

    pub fn record_metadata_contract_fetched(&self, contract_type: &str) {
        counter!("atlas_metadata_contracts_fetched_total", "type" => contract_type.to_string())
            .increment(1);
    }

    pub fn record_metadata_token_fetched(&self) {
        counter!("atlas_metadata_tokens_fetched_total").increment(1);
    }

    pub fn record_metadata_error(&self, metadata_type: &str) {
        counter!("atlas_metadata_errors_total", "type" => metadata_type.to_string()).increment(1);
    }

    // -- SSE helpers --

    pub fn increment_sse_connections(&self) {
        gauge!("atlas_sse_active_connections").increment(1.0);
    }

    pub fn decrement_sse_connections(&self) {
        gauge!("atlas_sse_active_connections").decrement(1.0);
    }

    // -- DB Pool helpers --

    pub fn set_db_pool_size(&self, pool_name: &str, size: f64) {
        gauge!("atlas_db_pool_size", "pool" => pool_name.to_string()).set(size);
    }

    pub fn set_db_pool_idle(&self, pool_name: &str, idle: f64) {
        gauge!("atlas_db_pool_idle", "pool" => pool_name.to_string()).set(idle);
    }

    pub fn set_db_pool_max(&self, pool_name: &str, max: f64) {
        gauge!("atlas_db_pool_max", "pool" => pool_name.to_string()).set(max);
    }

    // -- Error helper --

    /// Increment the error counter with component and error_type labels.
    /// Call this alongside `tracing::error!` / `tracing::warn!` at error sites.
    pub fn error(&self, component: &str, error_type: &str) {
        counter!(
            "atlas_errors_total",
            "component" => component.to_string(),
            "error_type" => error_type.to_string()
        )
        .increment(1);
    }
}

/// Guard that decrements the SSE connection gauge on drop.
pub struct SseConnectionGuard {
    metrics: Metrics,
}

impl SseConnectionGuard {
    pub fn new(metrics: Metrics) -> Self {
        metrics.increment_sse_connections();
        Self { metrics }
    }
}

impl Drop for SseConnectionGuard {
    fn drop(&mut self) {
        self.metrics.decrement_sse_connections();
    }
}

/// Axum middleware that records HTTP request metrics.
///
/// Uses `MatchedPath` to get the route pattern (e.g. `/api/blocks/{number}`)
/// instead of the concrete path, preventing label cardinality explosion.
pub async fn http_metrics_middleware(request: Request, next: Next) -> Response {
    let method = request.method().to_string();
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed = start.elapsed().as_secs_f64();

    let status = response.status().as_u16().to_string();

    counter!(
        "atlas_http_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status
    )
    .increment(1);

    histogram!(
        "atlas_http_request_duration_seconds",
        "method" => method,
        "path" => path
    )
    .record(elapsed);

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_new_does_not_panic_without_recorder() {
        // The metrics crate uses a no-op recorder by default,
        // so Metrics::new() should not panic even without install_prometheus_recorder().
        let _m = Metrics::new();
    }

    #[test]
    fn install_prometheus_recorder_is_idempotent() {
        let first = install_prometheus_recorder();
        let second = install_prometheus_recorder();
        let metrics = Metrics::new();
        metrics.set_indexer_head_block(1);

        assert!(first.render().contains("atlas_indexer_head_block"));
        assert!(second.render().contains("atlas_indexer_head_block"));
    }

    #[test]
    fn new_metrics_render_when_emitted() {
        let handle = install_prometheus_recorder();
        let metrics = Metrics::new();
        metrics.set_indexer_missing_blocks(3);
        metrics.set_indexer_head_block(42);
        metrics.set_indexer_head_block_timestamp(1_700_000_042);
        metrics.set_indexer_lag_blocks(7);
        metrics.record_block_processing_duration(0.1);

        let body = handle.render();
        assert!(body.contains("atlas_indexer_missing_blocks"));
        assert!(body.contains("atlas_indexer_head_block_timestamp_seconds"));
        assert!(body.contains("atlas_indexer_lag_blocks"));
        assert!(body.contains("atlas_indexer_block_processing_duration_seconds"));
    }
}
