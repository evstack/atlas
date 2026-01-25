use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod indexer;
mod metadata;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "atlas_indexer=info,sqlx=warn".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Atlas Indexer");

    // Load configuration
    dotenvy::dotenv().ok();
    let config = config::Config::from_env()?;

    // Create database pool
    let pool = atlas_common::db::create_pool(&config.database_url, config.db_max_connections).await?;

    // Run migrations
    tracing::info!("Running database migrations");
    atlas_common::db::run_migrations(&pool).await?;

    // Start indexer
    let indexer = indexer::Indexer::new(pool.clone(), config.clone());

    // Start metadata fetcher in background
    let metadata_pool = pool.clone();
    let metadata_config = config.clone();
    let metadata_handle = tokio::spawn(async move {
        metadata::MetadataFetcher::new(metadata_pool, metadata_config)
            .run()
            .await
    });

    // Run indexer (blocks until shutdown)
    indexer.run().await?;

    // Wait for metadata fetcher
    metadata_handle.await??;

    Ok(())
}
