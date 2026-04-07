use axum::Router;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::{Arc, LazyLock};
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::broadcast;

use atlas_server::api::{build_router, AppState};
use atlas_server::head::HeadTracker;

struct TestEnv {
    runtime: tokio::runtime::Runtime,
    pool: PgPool,
    _container: ContainerAsync<Postgres>,
}

// Single LazyLock: runtime + container + pool, all initialized together.
static ENV: LazyLock<TestEnv> = LazyLock::new(|| {
    let runtime = tokio::runtime::Runtime::new().expect("create test runtime");

    let (pool, container) = runtime.block_on(async {
        let container = Postgres::default()
            .start()
            .await
            .expect("Failed to start Postgres container");

        let host = container.get_host().await.expect("get host");
        let port = container.get_host_port_ipv4(5432).await.expect("get port");

        let database_url = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await
            .expect("Failed to create pool");

        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        (pool, container)
    });

    TestEnv {
        runtime,
        pool,
        _container: container,
    }
});

pub fn pool() -> &'static PgPool {
    &ENV.pool
}

pub fn test_router() -> Router {
    let pool = pool().clone();
    let head_tracker = Arc::new(HeadTracker::empty(10));
    let (tx, _) = broadcast::channel(1);
    let (da_tx, _) = broadcast::channel(1);

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
    });

    build_router(state, None)
}

/// Run an async test block on the shared runtime.
pub fn run<F: std::future::Future<Output = ()>>(f: F) {
    ENV.runtime.block_on(f);
}

/// Helper to parse a JSON response body.
pub async fn json_body(response: axum::http::Response<axum::body::Body>) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("parse JSON")
}
