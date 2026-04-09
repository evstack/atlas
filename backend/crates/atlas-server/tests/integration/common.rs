use axum::Router;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use std::{env, process::Command};
use testcontainers::runners::SyncRunner;
use testcontainers::{Container, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::sync::broadcast;

use atlas_server::api::{build_router, AppState};
use atlas_server::head::HeadTracker;

struct TestEnv {
    database_url: String,
    _container: Option<Container<Postgres>>,
}

fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn init_env() -> Result<TestEnv, String> {
    let (database_url, container) = if let Ok(database_url) = env::var("ATLAS_TEST_DATABASE_URL") {
        (database_url, None)
    } else if docker_available() {
        let container = Postgres::default()
            .with_startup_timeout(Duration::from_secs(180))
            .start()
            .map_err(|error| format!("failed to start Postgres test container: {error}"))?;
        let host = container
            .get_host()
            .map_err(|error| format!("failed to get test container host: {error}"))?;
        let port = container
            .get_host_port_ipv4(5432)
            .map_err(|error| format!("failed to get test container port: {error}"))?;
        (
            format!("postgres://postgres:postgres@{}:{}/postgres", host, port),
            Some(container),
        )
    } else {
        return Err("Docker is unavailable and ATLAS_TEST_DATABASE_URL is not set".to_string());
    };

    tokio::runtime::Runtime::new()
        .expect("create migration runtime")
        .block_on(async {
            let pool = PgPoolOptions::new()
                .max_connections(10)
                .connect(&database_url)
                .await
                .map_err(|error| format!("failed to create test pool: {error}"))?;

            sqlx::migrate!("../../migrations")
                .run(&pool)
                .await
                .map_err(|error| format!("failed to run test migrations: {error}"))?;

            pool.close().await;
            Ok::<(), String>(())
        })?;

    Ok(TestEnv {
        database_url,
        _container: container,
    })
}

// Single LazyLock: test database configuration, shared across tests.
static ENV: LazyLock<Result<TestEnv, String>> = LazyLock::new(init_env);

pub fn pool() -> PgPool {
    let env = ENV.as_ref().expect("integration test environment");
    PgPoolOptions::new()
        .max_connections(10)
        .connect_lazy(&env.database_url)
        .expect("create lazy pool")
}

pub fn test_router() -> Router {
    let pool = pool();
    let head_tracker = Arc::new(HeadTracker::empty(10));
    let (tx, _) = broadcast::channel(1);
    let (da_tx, _) = broadcast::channel(1);

    let prometheus_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .build_recorder()
        .handle();
    let state = Arc::new(AppState {
        pool,
        block_events_tx: tx,
        da_events_tx: da_tx,
        head_tracker,
        rpc_url: String::new(),
        da_tracking_enabled: false,
        faucet: None,
        chain_id: 42,
        chain_name: "Test Chain".to_string(),
        chain_logo_url: None,
        chain_logo_url_light: None,
        chain_logo_url_dark: None,
        accent_color: None,
        background_color_dark: None,
        background_color_light: None,
        success_color: None,
        error_color: None,
        metrics: atlas_server::metrics::Metrics::new(),
        prometheus_handle,
        solc_cache_dir: "/tmp/solc-cache".to_string(),
    });

    build_router(state, None)
}

/// Run an async test block when the integration database is available.
pub fn run<F: std::future::Future<Output = ()>>(f: F) {
    if let Err(error) = ENV.as_ref() {
        eprintln!("skipping integration test: {error}");
        return;
    }

    tokio::runtime::Runtime::new()
        .expect("create test runtime")
        .block_on(f);
}

/// Helper to parse a JSON response body.
pub async fn json_body(response: axum::http::Response<axum::body::Body>) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("parse JSON")
}
