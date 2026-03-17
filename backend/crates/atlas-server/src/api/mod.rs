pub mod error;
pub mod handlers;

use axum::{routing::get, Router};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::head::HeadTracker;

pub struct AppState {
    pub pool: PgPool,
    pub block_events_tx: broadcast::Sender<()>,
    pub head_tracker: Arc<HeadTracker>,
    pub rpc_url: String,
}

/// Build the Axum router.
///
/// `cors_origin`: when `Some`, restrict CORS to that exact origin; when `None`,
/// allow any origin (backwards-compatible default for development / self-hosted
/// deployments).
///
/// NOTE: Rate limiting has not yet been added here. The `tower_governor` crate
/// (v0.8, backed by `governor` v0.10) is incompatible with the `governor` v0.6
/// already used by the indexer. Once the indexer's governor dependency is
/// upgraded to v0.10, add a `GovernorLayer` with a per-IP burst of ~50 req/s.
pub fn build_router(state: Arc<AppState>, cors_origin: Option<String>) -> Router {
    // SSE route — excluded from TimeoutLayer so connections stay alive
    let sse_routes = Router::new()
        .route("/api/events", get(handlers::sse::block_events))
        .with_state(state.clone());

    Router::new()
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
        // Etherscan-compatible API
        .route("/api", get(handlers::etherscan::etherscan_api))
        // Search
        .route("/api/search", get(handlers::search::search))
        // Status
        .route("/api/status", get(handlers::status::get_status))
        // Health
        .route("/health", get(|| async { "OK" }))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .with_state(state)
        // Merge SSE routes (no TimeoutLayer so connections stay alive)
        .merge(sse_routes)
        // Shared layers applied to all routes
        .layer(build_cors_layer(cors_origin))
        .layer(TraceLayer::new_for_http())
}

/// Construct the CORS layer.
///
/// When `cors_origin` is `Some`, restrict to that exact origin.
/// When `None`, allow any origin so that self-hosted and development deployments
/// work out of the box without requiring the env var.
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
