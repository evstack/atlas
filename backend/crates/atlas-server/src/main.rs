use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod config;
mod indexer;

/// Retry delays for exponential backoff (in seconds)
const RETRY_DELAYS: &[u64] = &[5, 10, 20, 30, 60];
const MAX_RETRY_DELAY: u64 = 60;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "atlas_server=info,tower_http=debug,sqlx=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Atlas Server");

    // Load configuration
    dotenvy::dotenv().ok();
    let config = config::Config::from_env()?;

    // Run migrations once (dedicated pool, no statement_timeout)
    tracing::info!("Running database migrations");
    atlas_common::db::run_migrations(&config.database_url).await?;

    // Create separate DB pools for indexer and API
    let indexer_pool =
        atlas_common::db::create_pool(&config.database_url, config.indexer_db_max_connections)
            .await?;
    let api_pool =
        atlas_common::db::create_pool(&config.database_url, config.api_db_max_connections).await?;

    // Shared broadcast channel for SSE notifications
    let (block_events_tx, _) = broadcast::channel(1024);

    // Build AppState for API
    let state = Arc::new(api::AppState {
        pool: api_pool,
        block_events_tx: block_events_tx.clone(),
        rpc_url: config.rpc_url.clone(),
    });

    // Spawn indexer task with retry logic
    let indexer = indexer::Indexer::new(indexer_pool.clone(), config.clone(), block_events_tx);
    tokio::spawn(async move {
        if let Err(e) = run_with_retry(|| indexer.run()).await {
            tracing::error!("Indexer terminated with error: {}", e);
        }
    });

    // Spawn metadata fetcher in background
    let metadata_pool = indexer_pool;
    let metadata_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = run_with_retry(|| async {
            let fetcher =
                indexer::MetadataFetcher::new(metadata_pool.clone(), metadata_config.clone())?;
            fetcher.run().await
        })
        .await
        {
            tracing::error!("Metadata fetcher terminated with error: {}", e);
        }
    });

    // Build and serve API
    let app = api::build_router(state);
    let addr = format!("{}:{}", config.api_host, config.api_port);
    tracing::info!("API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl-c");
    tracing::info!("Shutdown signal received, stopping...");
}

/// Run an async function with exponential backoff retry
async fn run_with_retry<F, Fut>(f: F) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let mut retry_count = 0;

    loop {
        match f().await {
            Ok(()) => {
                retry_count = 0;
            }
            Err(e) => {
                let delay = RETRY_DELAYS
                    .get(retry_count)
                    .copied()
                    .unwrap_or(MAX_RETRY_DELAY);

                tracing::error!(
                    "Fatal error (internal retries exhausted): {}. Restarting in {}s (attempt {})...",
                    e,
                    delay,
                    retry_count + 1
                );

                tokio::time::sleep(Duration::from_secs(delay)).await;
                retry_count += 1;
            }
        }
    }
}
