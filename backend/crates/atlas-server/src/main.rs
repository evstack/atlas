use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;

mod api;
mod config;
mod faucet;
mod head;
mod indexer;

/// Retry delays for exponential backoff (in seconds)
const RETRY_DELAYS: &[u64] = &[5, 10, 20, 30, 60];
const MAX_RETRY_DELAY: u64 = 60;

fn parse_chain_id(hex: &str) -> Option<u64> {
    u64::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
}

async fn fetch_chain_id(rpc_url: &str) -> Result<u64> {
    let client = reqwest::Client::new();
    let resp = client
        .post(rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_chainId",
            "params": [],
            "id": 1
        }))
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?;

    let json: serde_json::Value = resp.json().await?;
    let hex = json["result"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("eth_chainId result missing"))?;
    parse_chain_id(hex).ok_or_else(|| anyhow::anyhow!("invalid eth_chainId hex"))
}

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
    let faucet_config = config::FaucetConfig::from_env()?;

    let faucet = if faucet_config.enabled {
        tracing::info!("Faucet enabled");
        let private_key = faucet_config
            .private_key
            .as_ref()
            .expect("validated faucet private key");
        let signer: PrivateKeySigner = private_key.parse().expect("validated faucet private key");
        let rpc_url: reqwest::Url = config
            .rpc_url
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid RPC_URL for faucet: {e}"))?;
        let provider = ProviderBuilder::new().wallet(signer).connect_http(rpc_url);
        Some(Arc::new(faucet::FaucetService::new(
            provider,
            faucet_config.amount_wei.expect("validated faucet amount"),
            faucet_config
                .cooldown_minutes
                .expect("validated faucet cooldown"),
        )) as Arc<dyn faucet::FaucetBackend>)
    } else {
        None
    };

    tracing::info!("Fetching chain ID from RPC");
    let chain_id = fetch_chain_id(&config.rpc_url).await?;
    tracing::info!("Chain ID: {}", chain_id);

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
    let head_tracker = Arc::new(if config.reindex {
        head::HeadTracker::empty(config.sse_replay_buffer_blocks)
    } else {
        head::HeadTracker::bootstrap(&api_pool, config.sse_replay_buffer_blocks).await?
    });

    // Build AppState for API
    let state = Arc::new(api::AppState {
        pool: api_pool,
        block_events_tx: block_events_tx.clone(),
        head_tracker: head_tracker.clone(),
        rpc_url: config.rpc_url.clone(),
        faucet,
        chain_id,
        chain_name: config.chain_name.clone(),
    });

    // Spawn indexer task with retry logic
    let indexer = indexer::Indexer::new(
        indexer_pool.clone(),
        config.clone(),
        block_events_tx,
        head_tracker,
    );
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
    let app = api::build_router(state, config.cors_origin.clone());
    let addr = format!("{}:{}", config.api_host, config.api_port);
    tracing::info!("API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to listen for SIGTERM");

        wait_for_shutdown_signal(
            async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed to listen for ctrl-c");
            },
            async move {
                terminate.recv().await;
            },
        )
        .await;
    }

    #[cfg(not(unix))]
    {
        wait_for_shutdown_signal(
            async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed to listen for ctrl-c");
            },
            std::future::pending::<()>(),
        )
        .await;
    }

    tracing::info!("Shutdown signal received, stopping...");
}

async fn wait_for_shutdown_signal<CtrlC, Term, CtrlOut, TermOut>(ctrl_c: CtrlC, terminate: Term)
where
    CtrlC: std::future::Future<Output = CtrlOut>,
    Term: std::future::Future<Output = TermOut>,
{
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    async fn serve_json_once(body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0_u8; 1024];
            let _ = socket.read(&mut buf).await.unwrap();

            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        format!("http://{}", addr)
    }

    #[tokio::test]
    async fn wait_for_shutdown_signal_returns_on_ctrl_c_future() {
        let (ctrl_tx, ctrl_rx) = oneshot::channel::<()>();
        let (_term_tx, term_rx) = oneshot::channel::<()>();

        let shutdown = tokio::spawn(wait_for_shutdown_signal(
            async move {
                let _ = ctrl_rx.await;
            },
            async move {
                let _ = term_rx.await;
            },
        ));

        ctrl_tx.send(()).unwrap();
        shutdown.await.unwrap();
    }

    #[tokio::test]
    async fn wait_for_shutdown_signal_returns_on_terminate_future() {
        let (_ctrl_tx, ctrl_rx) = oneshot::channel::<()>();
        let (term_tx, term_rx) = oneshot::channel::<()>();

        let shutdown = tokio::spawn(wait_for_shutdown_signal(
            async move {
                let _ = ctrl_rx.await;
            },
            async move {
                let _ = term_rx.await;
            },
        ));

        term_tx.send(()).unwrap();
        shutdown.await.unwrap();
    }

    #[tokio::test]
    async fn fetch_chain_id_reads_hex_result_from_rpc_response() {
        let url = serve_json_once(r#"{"jsonrpc":"2.0","id":1,"result":"0xa4b1"}"#).await;
        assert_eq!(fetch_chain_id(&url).await.unwrap(), 42161);
    }

    #[tokio::test]
    async fn fetch_chain_id_returns_error_for_invalid_result() {
        let url = serve_json_once(r#"{"jsonrpc":"2.0","id":1,"result":"not_hex"}"#).await;
        let err = fetch_chain_id(&url).await.unwrap_err();
        assert!(err.to_string().contains("invalid eth_chainId hex"));
    }

    #[tokio::test]
    async fn fetch_chain_id_returns_error_for_http_failure() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let url = format!("http://{}", addr);
        assert!(fetch_chain_id(&url).await.is_err());
    }
}
