use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::Client as S3Client;
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::sync::broadcast;

use atlas_server::api::{build_router, AppState};
use atlas_server::archive::S3ArchiveStore;
use atlas_server::config::ArchiveConfig;
use atlas_server::head::HeadTracker;

const MINIO_IMAGE_TAG: &str = "RELEASE.2024-01-16T16-07-38Z";
const MINIO_ROOT_USER: &str = "minioadmin";
const MINIO_ROOT_PASSWORD: &str = "minioadmin";
const MINIO_REGION: &str = "us-east-1";

struct TestEnv {
    runtime: tokio::runtime::Runtime,
    pool: PgPool,
    database_url: String,
    minio_endpoint: String,
    _postgres: ContainerAsync<Postgres>,
    _minio: ContainerAsync<GenericImage>,
}

// Single LazyLock: runtime + container + pool, all initialized together.
static ENV: LazyLock<TestEnv> = LazyLock::new(|| {
    let runtime = tokio::runtime::Runtime::new().expect("create test runtime");

    let (pool, postgres, minio, minio_endpoint, pg_database_url) = runtime.block_on(async {
        let postgres = Postgres::default()
            .start()
            .await
            .expect("Failed to start Postgres container");

        let host = postgres.get_host().await.expect("get host");
        let port = postgres.get_host_port_ipv4(5432).await.expect("get port");

        let pg_database_url = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(&pg_database_url)
            .await
            .expect("Failed to create pool");

        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        let minio = GenericImage::new("minio/minio", MINIO_IMAGE_TAG)
            .with_exposed_port(9000.tcp())
            .with_exposed_port(9001.tcp())
            .with_wait_for(WaitFor::seconds(2))
            .with_env_var("MINIO_ROOT_USER", MINIO_ROOT_USER)
            .with_env_var("MINIO_ROOT_PASSWORD", MINIO_ROOT_PASSWORD)
            .with_cmd([
                "server",
                "/data",
                "--address",
                ":9000",
                "--console-address",
                ":9001",
            ])
            .start()
            .await
            .expect("Failed to start MinIO container");

        let minio_host = minio.get_host().await.expect("get minio host");
        let minio_port = minio
            .get_host_port_ipv4(9000.tcp())
            .await
            .expect("get minio port");
        let minio_endpoint = format!("http://{}:{}", minio_host, minio_port);

        wait_for_minio(&minio_endpoint).await;

        (pool, postgres, minio, minio_endpoint, pg_database_url)
    });

    TestEnv {
        runtime,
        pool,
        database_url: pg_database_url,
        minio_endpoint,
        _postgres: postgres,
        _minio: minio,
    }
});

async fn wait_for_minio(endpoint: &str) {
    let client = reqwest::Client::new();

    for _ in 0..30 {
        let response = client
            .get(format!("{endpoint}/minio/health/live"))
            .timeout(Duration::from_secs(1))
            .send()
            .await;
        if let Ok(response) = response {
            if response.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    panic!("MinIO did not become ready at {endpoint}");
}

pub fn pool() -> &'static PgPool {
    &ENV.pool
}

pub fn database_url() -> &'static str {
    &ENV.database_url
}

pub fn minio_endpoint() -> &'static str {
    &ENV.minio_endpoint
}

pub fn archive_config(bucket: impl Into<String>) -> ArchiveConfig {
    ArchiveConfig {
        bucket: bucket.into(),
        region: MINIO_REGION.to_string(),
        prefix: "integration".to_string(),
        endpoint: Some(minio_endpoint().to_string()),
        force_path_style: true,
        upload_concurrency: 1,
        retry_base_seconds: 1,
    }
}

pub async fn archive_store(bucket: impl Into<String>) -> S3ArchiveStore {
    let bucket = bucket.into();
    let client = s3_client().await;
    let store = S3ArchiveStore::new(client, &bucket);
    atlas_server::archive::ArchiveObjectStore::ensure_bucket_access(&store)
        .await
        .expect("bucket access");
    store
}

pub async fn s3_client() -> S3Client {
    let creds = Credentials::new(MINIO_ROOT_USER, MINIO_ROOT_PASSWORD, None, None, "test");
    let s3_config = aws_sdk_s3::config::Builder::new()
        .region(Region::new(MINIO_REGION.to_string()))
        .endpoint_url(minio_endpoint())
        .credentials_provider(creds)
        .force_path_style(true)
        .behavior_version(BehaviorVersion::latest())
        .build();
    S3Client::from_conf(s3_config)
}

pub async fn create_bucket(bucket: &str) {
    let client = s3_client().await;
    let _ = client.create_bucket().bucket(bucket).send().await;
}

pub async fn get_object_bytes(bucket: &str, key: &str) -> Vec<u8> {
    let client = s3_client().await;
    let response = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .expect("get object");
    response
        .body
        .collect()
        .await
        .expect("collect object body")
        .into_bytes()
        .to_vec()
}

pub async fn reset_archive_tables() {
    sqlx::query("TRUNCATE archive_blocks, archive_state CASCADE")
        .execute(pool())
        .await
        .expect("truncate archive tables");
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
