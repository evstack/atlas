use anyhow::Result;
use axum::{
    routing::{get, post, delete},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod handlers;
mod error;

pub struct AppState {
    pub pool: PgPool,
    pub rpc_url: String,
    pub solc_path: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "atlas_api=info,tower_http=debug,sqlx=warn".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Atlas API Server");

    // Load configuration
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set");
    let solc_path = std::env::var("SOLC_PATH").unwrap_or_else(|_| "solc".to_string());
    let host = std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("API_PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("Invalid API_PORT");

    // Create database pool
    let pool = atlas_common::db::create_pool(&database_url, 20).await?;

    // Run migrations
    tracing::info!("Running database migrations");
    atlas_common::db::run_migrations(&pool).await?;

    let state = Arc::new(AppState { pool, rpc_url, solc_path });

    // Build router
    let app = Router::new()
        // Blocks
        .route("/api/blocks", get(handlers::blocks::list_blocks))
        .route("/api/blocks/{number}", get(handlers::blocks::get_block))
        .route("/api/blocks/{number}/transactions", get(handlers::blocks::get_block_transactions))
        // Transactions
        .route("/api/transactions/{hash}", get(handlers::transactions::get_transaction))
        .route("/api/transactions/{hash}/logs", get(handlers::logs::get_transaction_logs))
        .route("/api/transactions/{hash}/logs/decoded", get(handlers::logs::get_transaction_logs_decoded))
        // Addresses
        .route("/api/addresses/{address}", get(handlers::addresses::get_address))
        .route("/api/addresses/{address}/transactions", get(handlers::addresses::get_address_transactions))
        .route("/api/addresses/{address}/nfts", get(handlers::addresses::get_address_nfts))
        .route("/api/addresses/{address}/tokens", get(handlers::tokens::get_address_tokens))
        .route("/api/addresses/{address}/logs", get(handlers::logs::get_address_logs))
        .route("/api/addresses/{address}/label", get(handlers::labels::get_address_with_label))
        // NFTs
        .route("/api/nfts/collections", get(handlers::nfts::list_collections))
        .route("/api/nfts/collections/{address}", get(handlers::nfts::get_collection))
        .route("/api/nfts/collections/{address}/tokens", get(handlers::nfts::list_collection_tokens))
        .route("/api/nfts/collections/{address}/tokens/{token_id}", get(handlers::nfts::get_token))
        .route("/api/nfts/collections/{address}/tokens/{token_id}/transfers", get(handlers::nfts::get_token_transfers))
        // ERC-20 Tokens
        .route("/api/tokens", get(handlers::tokens::list_tokens))
        .route("/api/tokens/{address}", get(handlers::tokens::get_token))
        .route("/api/tokens/{address}/holders", get(handlers::tokens::get_token_holders))
        .route("/api/tokens/{address}/transfers", get(handlers::tokens::get_token_transfers))
        // Event Logs
        .route("/api/logs", get(handlers::logs::get_logs_by_topic))
        // Address Labels
        .route("/api/labels", get(handlers::labels::list_labels))
        .route("/api/labels", post(handlers::labels::upsert_label))
        .route("/api/labels/bulk", post(handlers::labels::bulk_import_labels))
        .route("/api/labels/tags", get(handlers::labels::list_tags))
        .route("/api/labels/{address}", get(handlers::labels::get_label))
        .route("/api/labels/{address}", delete(handlers::labels::delete_label))
        // Proxy Contracts
        .route("/api/proxies", get(handlers::proxy::list_proxies))
        .route("/api/contracts/{address}/proxy", get(handlers::proxy::get_proxy_info))
        .route("/api/contracts/{address}/combined-abi", get(handlers::proxy::get_combined_abi))
        .route("/api/contracts/{address}/detect-proxy", post(handlers::proxy::detect_proxy))
        // Contract Verification
        .route("/api/contracts/verify", post(handlers::contracts::verify_contract))
        .route("/api/contracts/{address}/abi", get(handlers::contracts::get_contract_abi))
        .route("/api/contracts/{address}/source", get(handlers::contracts::get_contract_source))
        // Etherscan-compatible API
        .route("/api", get(handlers::etherscan::etherscan_api))
        .route("/api", post(handlers::etherscan::etherscan_api_post))
        // Search
        .route("/api/search", get(handlers::search::search))
        // Health
        .route("/health", get(|| async { "OK" }))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
