use anyhow::Result;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod batch;
mod config;
mod copy;
mod da_worker;
mod evnode;
mod fetcher;
mod indexer;
mod metadata;

/// Retry delays for exponential backoff (in seconds)
const RETRY_DELAYS: &[u64] = &[5, 10, 20, 30, 60];
const MAX_RETRY_DELAY: u64 = 60;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "atlas_indexer=info,sqlx=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Atlas Indexer");

    // Load configuration
    dotenvy::dotenv().ok();
    let config = config::Config::from_env()?;

    // Create database pool
    let pool =
        atlas_common::db::create_pool(&config.database_url, config.db_max_connections).await?;

    // Run migrations
    tracing::info!("Running database migrations");
    atlas_common::db::run_migrations(&config.database_url).await?;

    // Start indexer
    let indexer = indexer::Indexer::new(pool.clone(), config.clone());

    // Start metadata fetcher in background
    let metadata_pool = pool.clone();
    let metadata_config = config.clone();
    let metadata_handle = tokio::spawn(async move {
        run_with_retry(|| async {
            let fetcher =
                metadata::MetadataFetcher::new(metadata_pool.clone(), metadata_config.clone())?;
            fetcher.run().await
        })
        .await
    });

    // Start DA worker in background (only if EVNODE_URL is configured)
    if let Some(ref evnode_url) = config.evnode_url {
        tracing::info!("DA worker enabled, ev-node URL: {}", evnode_url);
        let da_pool = pool.clone();
        let da_url = evnode_url.clone();
        let da_concurrency = config.da_worker_concurrency;
        tokio::spawn(async move {
            run_with_retry(|| async {
                let worker =
                    da_worker::DaWorker::new(da_pool.clone(), &da_url, da_concurrency)?;
                worker.run().await
            })
            .await
        });
    } else {
        tracing::info!("DA worker disabled (EVNODE_URL not set)");
    }

    // Run indexer with retry on failure
    run_with_retry(|| indexer.run()).await?;

    // Wait for metadata fetcher
    metadata_handle.await??;

    Ok(())
}

/// Run an async function with exponential backoff retry
/// Note: Network errors are handled internally by the indexer with their own retry logic.
/// This outer retry is for catastrophic errors (DB failures, all RPC retries exhausted, etc.)
async fn run_with_retry<F, Fut>(f: F) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let mut retry_count = 0;

    loop {
        match f().await {
            Ok(()) => {
                // Success - reset retry count and continue
                retry_count = 0;
            }
            Err(e) => {
                // Get delay for this retry (cap at MAX_RETRY_DELAY)
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
