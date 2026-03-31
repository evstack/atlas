use anyhow::{bail, Context, Result};
use clap::Parser;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;

mod api;
mod cli;
mod config;
mod faucet;
mod head;
mod indexer;
mod snapshot;

/// Retry delays for exponential backoff (in seconds)
const RETRY_DELAYS: &[u64] = &[5, 10, 20, 30, 60];
const MAX_RETRY_DELAY: u64 = 60;
const PORTABLE_PG_DUMP_FLAGS: &[&str] = &["--format=custom", "--no-owner", "--no-acl"];
const PORTABLE_PG_RESTORE_FLAGS: &[&str] = &[
    "--format=custom",
    "--no-owner",
    "--no-acl",
    "--exit-on-error",
];
const RESET_DB_FOR_RESTORE_SQL: &str = "DROP SCHEMA public CASCADE; CREATE SCHEMA public;";

fn init_tracing(filter: &str) {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn required_db_url(db_url: &str) -> Result<&str> {
    let db_url = db_url.trim();
    if db_url.is_empty() {
        bail!("DATABASE_URL must be set");
    }
    Ok(db_url)
}

pub(crate) struct PostgresConnectionConfig {
    pub(crate) database_name: String,
    pub(crate) env_vars: Vec<(&'static str, String)>,
}

const PG_ENV_VARS: &[&str] = &[
    "PGHOST",
    "PGHOSTADDR",
    "PGPORT",
    "PGUSER",
    "PGPASSWORD",
    "PGDATABASE",
    "PGSERVICE",
    "PGSSLMODE",
    "PGSSLCERT",
    "PGSSLKEY",
    "PGSSLROOTCERT",
    "PGSSLCRL",
    "PGAPPNAME",
    "PGOPTIONS",
    "PGCONNECT_TIMEOUT",
];

fn set_pg_env(env_vars: &mut Vec<(&'static str, String)>, key: &'static str, value: &str) {
    if value.is_empty() {
        return;
    }
    if let Some((_, existing)) = env_vars
        .iter_mut()
        .find(|(existing_key, _)| *existing_key == key)
    {
        *existing = value.to_string();
    } else {
        env_vars.push((key, value.to_string()));
    }
}

pub(crate) fn postgres_connection_config(db_url: &str) -> Result<PostgresConnectionConfig> {
    let url = reqwest::Url::parse(required_db_url(db_url)?).context("Invalid DATABASE_URL")?;
    match url.scheme() {
        "postgres" | "postgresql" => {}
        _ => bail!("DATABASE_URL must use postgres:// or postgresql://"),
    }

    let mut database_name = url.path().trim_start_matches('/').to_string();
    let mut env_vars = Vec::new();
    if let Some(host) = url.host_str() {
        set_pg_env(&mut env_vars, "PGHOST", host);
    }
    if let Some(port) = url.port() {
        set_pg_env(&mut env_vars, "PGPORT", &port.to_string());
    }
    if !url.username().is_empty() {
        set_pg_env(&mut env_vars, "PGUSER", url.username());
    }
    if let Some(password) = url.password() {
        set_pg_env(&mut env_vars, "PGPASSWORD", password);
    }

    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "dbname" => {
                if !value.is_empty() {
                    database_name = value.into_owned();
                }
            }
            "host" => {
                set_pg_env(&mut env_vars, "PGHOST", value.as_ref());
            }
            "hostaddr" => {
                set_pg_env(&mut env_vars, "PGHOSTADDR", value.as_ref());
            }
            "port" => {
                set_pg_env(&mut env_vars, "PGPORT", value.as_ref());
            }
            "user" => {
                set_pg_env(&mut env_vars, "PGUSER", value.as_ref());
            }
            "password" => {
                set_pg_env(&mut env_vars, "PGPASSWORD", value.as_ref());
            }
            "service" => {
                set_pg_env(&mut env_vars, "PGSERVICE", value.as_ref());
            }
            "sslmode" => {
                set_pg_env(&mut env_vars, "PGSSLMODE", value.as_ref());
            }
            "sslcert" => {
                set_pg_env(&mut env_vars, "PGSSLCERT", value.as_ref());
            }
            "sslkey" => {
                set_pg_env(&mut env_vars, "PGSSLKEY", value.as_ref());
            }
            "sslrootcert" => {
                set_pg_env(&mut env_vars, "PGSSLROOTCERT", value.as_ref());
            }
            "sslcrl" => {
                set_pg_env(&mut env_vars, "PGSSLCRL", value.as_ref());
            }
            "application_name" => {
                set_pg_env(&mut env_vars, "PGAPPNAME", value.as_ref());
            }
            "options" => {
                set_pg_env(&mut env_vars, "PGOPTIONS", value.as_ref());
            }
            "connect_timeout" => {
                set_pg_env(&mut env_vars, "PGCONNECT_TIMEOUT", value.as_ref());
            }
            _ => {}
        }
    }

    if database_name.is_empty() {
        bail!("DATABASE_URL must include a database name");
    }
    set_pg_env(&mut env_vars, "PGDATABASE", &database_name);

    Ok(PostgresConnectionConfig {
        database_name,
        env_vars,
    })
}

fn postgres_command(program: &str, config: &PostgresConnectionConfig) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    for env_var in PG_ENV_VARS {
        command.env_remove(env_var);
    }
    for (key, value) in &config.env_vars {
        command.env(key, value);
    }
    command
}

pub(crate) fn postgres_command_async(
    program: &str,
    config: &PostgresConnectionConfig,
) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    for env_var in PG_ENV_VARS {
        command.env_remove(env_var);
    }
    for (key, value) in &config.env_vars {
        command.env(key, value);
    }
    command
}

fn portable_pg_dump_command(
    program: &str,
    config: &PostgresConnectionConfig,
) -> std::process::Command {
    let mut command = postgres_command(program, config);
    command.args(PORTABLE_PG_DUMP_FLAGS);
    command
}

pub(crate) fn portable_pg_dump_command_async(
    program: &str,
    config: &PostgresConnectionConfig,
) -> tokio::process::Command {
    let mut command = postgres_command_async(program, config);
    command.args(PORTABLE_PG_DUMP_FLAGS);
    command
}

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
    // Load .env before clap so env vars are available for clap's `env = "..."` fallback
    dotenvy::dotenv().ok();

    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::Run(args) => run(*args).await,
        cli::Command::Migrate(args) => {
            init_tracing(&args.log.level);
            tracing::info!("Running database migrations");
            let database_url = required_db_url(&args.db.url)?;
            atlas_common::db::run_migrations(database_url).await?;
            tracing::info!("Migrations complete");
            Ok(())
        }
        cli::Command::Check(args) => check(*args).await,
        cli::Command::Db(db_cmd) => match db_cmd.command {
            cli::DbSubcommand::Dump { output, db_url } => cmd_db_dump(&db_url, &output),
            cli::DbSubcommand::Restore { input, db_url } => cmd_db_restore(&db_url, &input),
            cli::DbSubcommand::Reset { confirm, db_url } => cmd_db_reset(&db_url, confirm).await,
        },
    }
}

async fn run(args: cli::RunArgs) -> Result<()> {
    init_tracing(&args.log.level);
    tracing::info!("Starting Atlas Server");

    let config = config::Config::from_run_args(args.clone())?;
    let faucet_config = config::FaucetConfig::from_faucet_args(&args.faucet)?;
    let snapshot_config = config::SnapshotConfig::from_env(&config.database_url)?;

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
            .map_err(|e| anyhow::anyhow!("Invalid RPC URL for faucet: {e}"))?;
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

    tracing::info!("Running database migrations");
    atlas_common::db::run_migrations(&config.database_url).await?;

    let indexer_pool =
        atlas_common::db::create_pool(&config.database_url, config.indexer_db_max_connections)
            .await?;
    let api_pool =
        atlas_common::db::create_pool(&config.database_url, config.api_db_max_connections).await?;

    let (block_events_tx, _) = broadcast::channel(1024);
    let (da_events_tx, _) = broadcast::channel::<Vec<indexer::DaSseUpdate>>(256);
    let head_tracker = Arc::new(if config.reindex {
        head::HeadTracker::empty(config.sse_replay_buffer_blocks)
    } else {
        head::HeadTracker::bootstrap(&api_pool, config.sse_replay_buffer_blocks).await?
    });

    let state = Arc::new(api::AppState {
        pool: api_pool,
        block_events_tx: block_events_tx.clone(),
        da_events_tx: da_events_tx.clone(),
        head_tracker: head_tracker.clone(),
        rpc_url: config.rpc_url.clone(),
        da_tracking_enabled: config.da_tracking_enabled,
        faucet,
        chain_id,
        chain_name: config.chain_name.clone(),
        chain_logo_url: config.chain_logo_url.clone(),
        accent_color: config.accent_color.clone(),
        background_color_dark: config.background_color_dark.clone(),
        background_color_light: config.background_color_light.clone(),
        success_color: config.success_color.clone(),
        error_color: config.error_color.clone(),
    });

    let da_pool = indexer_pool.clone();
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

    if config.da_tracking_enabled {
        let evnode_url = config
            .evnode_url
            .as_deref()
            .expect("DA tracking requires EVNODE_URL");
        tracing::info!(
            "DA tracking enabled (workers: {}, rate_limit: {} req/s)",
            config.da_worker_concurrency,
            config.da_rpc_requests_per_second
        );
        let da_worker = indexer::DaWorker::new(
            da_pool,
            evnode_url,
            config.da_worker_concurrency,
            config.da_rpc_requests_per_second,
            da_events_tx,
        )?;
        tokio::spawn(async move {
            if let Err(e) = run_with_retry(|| da_worker.run()).await {
                tracing::error!("DA worker terminated with error: {}", e);
            }
        });
    }

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

    // Spawn snapshot scheduler if enabled
    if snapshot_config.enabled {
        tracing::info!("Snapshot scheduler enabled");
        tokio::spawn(async move {
            if let Err(e) =
                run_with_retry(|| snapshot::run_snapshot_loop(snapshot_config.clone())).await
            {
                tracing::error!("Snapshot scheduler terminated with error: {}", e);
            }
        });
    }

    let app = api::build_router(state, config.cors_origin.clone());
    let addr = format!("{}:{}", config.api_host, config.api_port);
    tracing::info!("API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn check(args: cli::RunArgs) -> Result<()> {
    init_tracing(&args.log.level);

    let config = config::Config::from_run_args(args.clone())?;
    config::FaucetConfig::from_faucet_args(&args.faucet)?;

    // Test DB connectivity
    tracing::info!("Testing database connectivity...");
    let pool = atlas_common::db::create_pool(&config.database_url, 1).await?;
    sqlx::query("SELECT 1").execute(&pool).await?;
    tracing::info!("Database OK");

    // Test RPC connectivity
    tracing::info!("Testing RPC connectivity...");
    let chain_id = fetch_chain_id(&config.rpc_url).await?;
    tracing::info!("RPC OK — chain_id={}", chain_id);

    tracing::info!("Configuration is valid");
    Ok(())
}

fn cmd_db_dump(db_url: &str, output: &str) -> Result<()> {
    let config = postgres_connection_config(db_url)?;
    let status = portable_pg_dump_command("pg_dump", &config)
        .args(["--file", output])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run pg_dump (is it installed?): {e}"))?;

    if !status.success() {
        anyhow::bail!("pg_dump exited with status {status}");
    }
    eprintln!("Dump written to {output}");
    Ok(())
}

fn cmd_db_restore(db_url: &str, input: &str) -> Result<()> {
    let config = postgres_connection_config(db_url)?;

    let reset_status = postgres_command("psql", &config)
        .arg("--dbname")
        .arg(&config.database_name)
        .arg("-v")
        .arg("ON_ERROR_STOP=1")
        .arg("-c")
        .arg(RESET_DB_FOR_RESTORE_SQL)
        .status()
        .map_err(|e| {
            anyhow::anyhow!("Failed to run psql before restore (is it installed?): {e}")
        })?;
    if !reset_status.success() {
        anyhow::bail!("psql exited with status {reset_status}");
    }

    let status = postgres_command("pg_restore", &config)
        .arg("--dbname")
        .arg(&config.database_name)
        .args(PORTABLE_PG_RESTORE_FLAGS)
        .arg(input)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run pg_restore (is it installed?): {e}"))?;

    if !status.success() {
        anyhow::bail!("pg_restore exited with status {status}");
    }
    eprintln!("Restore complete from {input}");
    Ok(())
}

async fn cmd_db_reset(db_url: &str, confirm: bool) -> Result<()> {
    if !confirm {
        eprintln!("This will DELETE all indexed data. Pass --confirm to proceed.");
        std::process::exit(1);
    }

    let pool = atlas_common::db::create_pool(required_db_url(db_url)?, 1).await?;
    sqlx::query(
        "TRUNCATE blocks, transactions, event_logs, addresses, nft_contracts, nft_tokens,
         nft_transfers, indexer_state, erc20_contracts, erc20_transfers, erc20_balances,
         event_signatures, address_labels, proxy_contracts, contract_abis, failed_blocks,
         tx_hash_lookup, block_da_status CASCADE",
    )
    .execute(&pool)
    .await?;
    eprintln!("All indexed data has been reset.");
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

    fn env_value<'a>(config: &'a PostgresConnectionConfig, key: &str) -> Option<&'a str> {
        config
            .env_vars
            .iter()
            .find(|(env_key, _)| *env_key == key)
            .map(|(_, value)| value.as_str())
    }

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

    #[test]
    fn postgres_connection_config_accepts_dbname_query_when_path_is_empty() {
        let config = postgres_connection_config(
            "postgres://user:secret@localhost?dbname=atlas&host=db.internal&port=6543&service=atlas-ci",
        )
        .unwrap();

        assert_eq!(config.database_name, "atlas");
        assert_eq!(env_value(&config, "PGHOST"), Some("db.internal"));
        assert_eq!(env_value(&config, "PGPORT"), Some("6543"));
        assert_eq!(env_value(&config, "PGUSER"), Some("user"));
        assert_eq!(env_value(&config, "PGPASSWORD"), Some("secret"));
        assert_eq!(env_value(&config, "PGSERVICE"), Some("atlas-ci"));
        assert_eq!(env_value(&config, "PGDATABASE"), Some("atlas"));
    }

    #[test]
    fn postgres_connection_config_query_params_override_url_components() {
        let config = postgres_connection_config(
            "postgres://user:secret@localhost/base_db?dbname=query_db&host=query-host&hostaddr=127.0.0.1&user=query-user&password=query-pass",
        )
        .unwrap();

        assert_eq!(config.database_name, "query_db");
        assert_eq!(env_value(&config, "PGHOST"), Some("query-host"));
        assert_eq!(env_value(&config, "PGHOSTADDR"), Some("127.0.0.1"));
        assert_eq!(env_value(&config, "PGUSER"), Some("query-user"));
        assert_eq!(env_value(&config, "PGPASSWORD"), Some("query-pass"));
        assert_eq!(env_value(&config, "PGDATABASE"), Some("query_db"));
    }

    #[test]
    fn portable_pg_dump_flags_omit_source_ownership_and_acls() {
        assert_eq!(
            PORTABLE_PG_DUMP_FLAGS,
            ["--format=custom", "--no-owner", "--no-acl"]
        );
    }

    #[test]
    fn portable_pg_restore_prepares_clean_schema_and_exits_on_first_error() {
        assert_eq!(
            PORTABLE_PG_RESTORE_FLAGS,
            [
                "--format=custom",
                "--no-owner",
                "--no-acl",
                "--exit-on-error"
            ]
        );
        assert_eq!(
            RESET_DB_FOR_RESTORE_SQL,
            "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
        );
    }
}
