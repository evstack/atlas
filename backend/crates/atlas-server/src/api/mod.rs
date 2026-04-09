pub mod error;
pub mod handlers;

use axum::{middleware, routing::get, Router};
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::faucet::SharedFaucetBackend;
use crate::head::HeadTracker;
use crate::indexer::DaSseUpdate;
use crate::metrics::Metrics;

pub struct AppState {
    pub pool: PgPool,
    pub block_events_tx: broadcast::Sender<()>,
    pub da_events_tx: broadcast::Sender<Vec<DaSseUpdate>>,
    pub head_tracker: Arc<HeadTracker>,
    pub rpc_url: String,
    pub da_tracking_enabled: bool,
    pub faucet: Option<SharedFaucetBackend>,
    pub chain_id: u64,
    pub chain_name: String,
    pub chain_logo_url: Option<String>,
    pub chain_logo_url_light: Option<String>,
    pub chain_logo_url_dark: Option<String>,
    pub accent_color: Option<String>,
    pub background_color_dark: Option<String>,
    pub background_color_light: Option<String>,
    pub success_color: Option<String>,
    pub error_color: Option<String>,
    pub metrics: Metrics,
    pub prometheus_handle: PrometheusHandle,
    pub solc_cache_dir: String,
}

/// Build the Axum router.
///
/// `cors_origin`: when `Some`, restrict CORS to that exact origin; when `None`,
/// allow any origin for development / self-hosted deployments.
pub fn build_router(state: Arc<AppState>, cors_origin: Option<String>) -> Router {
    // SSE route — excluded from TimeoutLayer so connections stay alive
    let sse_routes = Router::new()
        .route("/api/events", get(handlers::sse::block_events))
        .with_state(state.clone());

    // Verify route — excluded from 10s TimeoutLayer; solc compilation can take longer
    let verify_routes = Router::new()
        .route(
            "/api/contracts/{address}/verify",
            axum::routing::post(handlers::contracts::verify_contract),
        )
        .with_state(state.clone());

    let mut router = Router::new()
        // Blocks
        .route("/api/blocks", get(handlers::blocks::list_blocks))
        .route("/api/blocks/{number}", get(handlers::blocks::get_block))
        .route(
            "/api/blocks/{number}/transactions",
            get(handlers::blocks::get_block_transactions),
        )
        // Transactions
        .route(
            "/api/transactions",
            get(handlers::transactions::list_transactions),
        )
        .route(
            "/api/transactions/{hash}",
            get(handlers::transactions::get_transaction),
        )
        .route(
            "/api/transactions/{hash}/logs",
            get(handlers::logs::get_transaction_logs),
        )
        .route(
            "/api/transactions/{hash}/logs/decoded",
            get(handlers::logs::get_transaction_logs_decoded),
        )
        .route(
            "/api/transactions/{hash}/erc20-transfers",
            get(handlers::transactions::get_transaction_erc20_transfers),
        )
        .route(
            "/api/transactions/{hash}/nft-transfers",
            get(handlers::transactions::get_transaction_nft_transfers),
        )
        // Addresses
        .route("/api/addresses", get(handlers::addresses::list_addresses))
        .route(
            "/api/addresses/{address}",
            get(handlers::addresses::get_address),
        )
        .route(
            "/api/addresses/{address}/transactions",
            get(handlers::addresses::get_address_transactions),
        )
        .route(
            "/api/addresses/{address}/transfers",
            get(handlers::addresses::get_address_transfers),
        )
        .route(
            "/api/addresses/{address}/nfts",
            get(handlers::addresses::get_address_nfts),
        )
        .route(
            "/api/addresses/{address}/tokens",
            get(handlers::tokens::get_address_tokens),
        )
        .route(
            "/api/addresses/{address}/logs",
            get(handlers::logs::get_address_logs),
        )
        // NFTs
        .route(
            "/api/nfts/collections",
            get(handlers::nfts::list_collections),
        )
        .route(
            "/api/nfts/collections/{address}",
            get(handlers::nfts::get_collection),
        )
        .route(
            "/api/nfts/collections/{address}/tokens",
            get(handlers::nfts::list_collection_tokens),
        )
        .route(
            "/api/nfts/collections/{address}/transfers",
            get(handlers::nfts::get_collection_transfers),
        )
        .route(
            "/api/nfts/collections/{address}/tokens/{token_id}",
            get(handlers::nfts::get_token),
        )
        .route(
            "/api/nfts/collections/{address}/tokens/{token_id}/transfers",
            get(handlers::nfts::get_token_transfers),
        )
        // ERC-20 Tokens
        .route("/api/tokens", get(handlers::tokens::list_tokens))
        .route("/api/tokens/{address}", get(handlers::tokens::get_token))
        .route(
            "/api/tokens/{address}/holders",
            get(handlers::tokens::get_token_holders),
        )
        .route(
            "/api/tokens/{address}/transfers",
            get(handlers::tokens::get_token_transfers),
        )
        .route(
            "/api/tokens/{address}/chart",
            get(handlers::tokens::get_token_chart),
        )
        // Proxy Contracts
        .route("/api/proxies", get(handlers::proxy::list_proxies))
        .route(
            "/api/contracts/{address}/proxy",
            get(handlers::proxy::get_proxy_info),
        )
        .route(
            "/api/contracts/{address}/combined-abi",
            get(handlers::proxy::get_combined_abi),
        )
        // Contract verification
        .route(
            "/api/contracts/{address}",
            get(handlers::contracts::get_contract),
        )
        // Etherscan-compatible API
        .route("/api", get(handlers::etherscan::etherscan_api))
        // Search
        .route("/api/search", get(handlers::search::search))
        // Stats (charts)
        .route(
            "/api/stats/blocks-chart",
            get(handlers::stats::get_blocks_chart),
        )
        .route("/api/stats/daily-txs", get(handlers::stats::get_daily_txs))
        .route(
            "/api/stats/gas-price",
            get(handlers::stats::get_gas_price_chart),
        )
        // Status
        .route("/api/height", get(handlers::status::get_height))
        .route("/api/status", get(handlers::status::get_status))
        // Config (white-label branding)
        .route("/api/config", get(handlers::config::get_config))
        // Metrics
        .route("/metrics", get(handlers::metrics::metrics))
        // Health
        .route("/health", get(|| async { "OK" }))
        .route("/health/live", get(handlers::health::liveness))
        .route("/health/ready", get(handlers::health::readiness));

    if state.faucet.is_some() {
        router = router
            .route("/api/faucet/info", get(handlers::faucet::get_faucet_info))
            .route(
                "/api/faucet",
                axum::routing::post(handlers::faucet::request_faucet),
            );
    }

    router
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        // HTTP metrics middleware — placed after routing so MatchedPath is available
        .layer(middleware::from_fn(crate::metrics::http_metrics_middleware))
        // Merge SSE routes without TimeoutLayer so connections stay alive
        .merge(sse_routes)
        // Merge verify route without TimeoutLayer so solc compilation is not cut off
        .merge(verify_routes)
        // Shared layers applied to all routes
        .layer(build_cors_layer(cors_origin))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
/// Construct the CORS layer.
///
/// When `cors_origin` is `Some`, restrict to that exact origin.
/// When `None`, allow any origin.
fn build_cors_layer(cors_origin: Option<String>) -> CorsLayer {
    let origin = match cors_origin {
        Some(origin) => {
            let header_value = origin
                .parse::<axum::http::HeaderValue>()
                .expect("CORS_ORIGIN is not a valid HTTP header value");
            AllowOrigin::exact(header_value)
        }
        None => AllowOrigin::any(),
    };
    CorsLayer::new()
        .allow_origin(origin)
        .allow_methods(Any)
        .allow_headers(Any)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faucet::{FaucetBackend, FaucetInfo, FaucetTxResponse, SharedFaucetBackend};
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use futures::future::{BoxFuture, FutureExt};
    use tokio::sync::broadcast;
    use tower::util::ServiceExt;

    #[derive(Clone)]
    struct FakeFaucet;

    impl FaucetBackend for FakeFaucet {
        fn info(&self) -> BoxFuture<'static, Result<FaucetInfo, atlas_common::AtlasError>> {
            async move {
                Ok(FaucetInfo {
                    amount_wei: "1000".to_string(),
                    balance_wei: "2000".to_string(),
                    cooldown_minutes: 30,
                })
            }
            .boxed()
        }

        fn request_faucet(
            &self,
            _recipient: alloy::primitives::Address,
            _client_ip: String,
        ) -> BoxFuture<'static, Result<FaucetTxResponse, atlas_common::AtlasError>> {
            async move {
                Ok(FaucetTxResponse {
                    tx_hash: "0xdeadbeef".to_string(),
                })
            }
            .boxed()
        }
    }

    fn test_state(faucet: Option<SharedFaucetBackend>) -> Arc<AppState> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost:5432/test")
            .expect("lazy pool");
        let head_tracker = Arc::new(crate::head::HeadTracker::empty(10));
        let (tx, _) = broadcast::channel(1);
        let (da_tx, _) = broadcast::channel(1);
        let prometheus_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
            .build_recorder()
            .handle();
        Arc::new(AppState {
            pool,
            block_events_tx: tx,
            da_events_tx: da_tx,
            head_tracker,
            rpc_url: String::new(),
            da_tracking_enabled: false,
            faucet,
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

    #[tokio::test]
    async fn faucet_routes_are_not_mounted_when_disabled() {
        let app = build_router(test_state(None), None);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/faucet/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn faucet_routes_work_when_enabled() {
        let faucet: SharedFaucetBackend = Arc::new(FakeFaucet);
        let app = build_router(test_state(Some(faucet)), None);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/faucet")
                    .header("content-type", "application/json")
                    .header("x-forwarded-for", "127.0.0.1")
                    .body(Body::from(
                        r#"{"address":"0x0000000000000000000000000000000000000001"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["tx_hash"], "0xdeadbeef");
    }
}
