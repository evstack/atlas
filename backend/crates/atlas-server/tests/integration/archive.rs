use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use atlas_server::archive::{
    block_object_key, insert_archive_entries, manifest_object_key, ArchiveBundleV1, ArchiveEntry,
    ArchiveManifestV1, ArchiveObjectStore, ArchiveUploader, ARCHIVE_SCHEMA_VERSION,
};
use atlas_server::config::ArchiveConfig;

use crate::common;

static ARCHIVE_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static BUCKET_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn next_bucket_name() -> String {
    let id = BUCKET_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("atlas-archive-test-{id}")
}

async fn seed_archive_block(
    pool: &PgPool,
    prefix: &str,
    chain_id: u64,
    block_number: i64,
    archive_start_block: i64,
) {
    sqlx::query(
        "INSERT INTO archive_state
            (stream, archive_start_block, latest_contiguous_uploaded_block, schema_version, manifest_dirty, updated_at)
         VALUES ('blocks', $1, NULL, $2, FALSE, NOW())
         ON CONFLICT (stream) DO NOTHING",
    )
    .bind(archive_start_block)
    .bind(ARCHIVE_SCHEMA_VERSION)
    .execute(pool)
    .await
    .expect("insert archive state");

    let bundle = ArchiveBundleV1 {
        schema_version: ARCHIVE_SCHEMA_VERSION,
        chain_id,
        block_number: block_number as u64,
        block: alloy::rpc::types::Block::default(),
        receipts: Vec::new(),
    };
    let payload = zstd::stream::encode_all(
        serde_json::to_vec(&bundle)
            .expect("serialize bundle")
            .as_slice(),
        3,
    )
    .expect("compress bundle");

    sqlx::query(
        "INSERT INTO archive_blocks
            (block_number, object_key, payload, schema_version, retry_count, next_attempt_at, created_at, updated_at)
         VALUES ($1, $2, $3, $4, 0, NOW(), NOW(), NOW())",
    )
    .bind(block_number)
    .bind(block_object_key(prefix, block_number as u64))
    .bind(payload)
    .bind(ARCHIVE_SCHEMA_VERSION)
    .execute(pool)
    .await
    .expect("insert archive block");
}

fn archive_entry(prefix: &str, chain_id: u64, block_number: i64) -> ArchiveEntry {
    let bundle = ArchiveBundleV1 {
        schema_version: ARCHIVE_SCHEMA_VERSION,
        chain_id,
        block_number: block_number as u64,
        block: alloy::rpc::types::Block::default(),
        receipts: Vec::new(),
    };

    ArchiveEntry {
        block_number,
        object_key: block_object_key(prefix, block_number as u64),
        payload: zstd::stream::encode_all(
            serde_json::to_vec(&bundle)
                .expect("serialize bundle")
                .as_slice(),
            3,
        )
        .expect("compress bundle"),
        schema_version: ARCHIVE_SCHEMA_VERSION,
    }
}

async fn wait_until(
    timeout: Duration,
    mut predicate: impl FnMut() -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>,
) {
    let deadline = Instant::now() + timeout;
    loop {
        if predicate().await {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "condition timed out after {timeout:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[derive(Default)]
struct FailingStore;

#[async_trait]
impl ArchiveObjectStore for FailingStore {
    async fn ensure_bucket_access(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn put_archive_object(
        &self,
        _key: &str,
        _payload: Vec<u8>,
    ) -> anyhow::Result<Option<String>> {
        anyhow::bail!("simulated archive upload failure");
    }

    async fn put_manifest_object(
        &self,
        _key: &str,
        _payload: Vec<u8>,
    ) -> anyhow::Result<Option<String>> {
        Ok(Some("\"manifest\"".to_string()))
    }
}

#[derive(Default)]
struct FlakyStore {
    attempts: Mutex<HashMap<String, usize>>,
    manifests: Mutex<Vec<Vec<u8>>>,
}

#[async_trait]
impl ArchiveObjectStore for FlakyStore {
    async fn ensure_bucket_access(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn put_archive_object(
        &self,
        key: &str,
        _payload: Vec<u8>,
    ) -> anyhow::Result<Option<String>> {
        let mut attempts = self.attempts.lock().expect("lock attempts");
        let attempt = attempts.entry(key.to_string()).or_insert(0);
        *attempt += 1;
        if key.ends_with("/000000000031.json.zst") && *attempt == 1 {
            anyhow::bail!("simulated transient gap")
        }
        Ok(Some(format!("etag-{key}")))
    }

    async fn put_manifest_object(
        &self,
        _key: &str,
        payload: Vec<u8>,
    ) -> anyhow::Result<Option<String>> {
        self.manifests.lock().expect("lock manifests").push(payload);
        Ok(Some("\"manifest\"".to_string()))
    }
}

#[derive(Default)]
struct RecordingStore {
    manifests: Mutex<Vec<Vec<u8>>>,
}

#[async_trait]
impl ArchiveObjectStore for RecordingStore {
    async fn ensure_bucket_access(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn put_archive_object(
        &self,
        key: &str,
        _payload: Vec<u8>,
    ) -> anyhow::Result<Option<String>> {
        Ok(Some(format!("etag-{key}")))
    }

    async fn put_manifest_object(
        &self,
        _key: &str,
        payload: Vec<u8>,
    ) -> anyhow::Result<Option<String>> {
        self.manifests.lock().expect("lock manifests").push(payload);
        Ok(Some("\"manifest\"".to_string()))
    }
}

#[test]
fn successful_upload_to_minio_marks_uploaded_and_writes_manifest() {
    let _guard = ARCHIVE_TEST_LOCK.lock().expect("archive test lock");
    common::run(async {
        common::reset_archive_tables().await;

        let bucket = next_bucket_name();
        common::create_bucket(&bucket).await;
        let config = common::archive_config(&bucket);
        let prefix = config.prefix.clone();
        let store = Arc::new(common::archive_store(&bucket).await);

        seed_archive_block(common::pool(), &prefix, 42, 11, 11).await;

        let uploader = ArchiveUploader::new(
            common::pool().clone(),
            store.clone() as Arc<dyn ArchiveObjectStore>,
            config,
            42,
        );
        let handle = tokio::spawn(async move { uploader.run().await });

        wait_until(Duration::from_secs(10), || {
            Box::pin(async {
                let row: (Option<DateTime<Utc>>, Option<Vec<u8>>, Option<String>) = sqlx::query_as(
                    "SELECT uploaded_at, payload, etag FROM archive_blocks WHERE block_number = 11",
                )
                .fetch_one(common::pool())
                .await
                .expect("fetch archive row");
                let state: (Option<i64>, bool, Option<DateTime<Utc>>) = sqlx::query_as(
                    "SELECT latest_contiguous_uploaded_block, manifest_dirty, manifest_updated_at
                     FROM archive_state WHERE stream = 'blocks'",
                )
                .fetch_one(common::pool())
                .await
                .expect("fetch archive state");
                row.0.is_some()
                    && row.1.is_none()
                    && row.2.is_some()
                    && state.0 == Some(11)
                    && !state.1
                    && state.2.is_some()
            })
        })
        .await;

        let object_key = block_object_key(&prefix, 11);
        let object_bytes = common::get_object_bytes(&bucket, &object_key).await;
        let decoded =
            zstd::stream::decode_all(object_bytes.as_slice()).expect("decode archive object");
        let bundle: ArchiveBundleV1 =
            serde_json::from_slice(&decoded).expect("decode archive bundle");
        assert_eq!(bundle.chain_id, 42);
        assert_eq!(bundle.block_number, 11);

        let manifest_bytes = common::get_object_bytes(&bucket, &manifest_object_key(&prefix)).await;
        let manifest: ArchiveManifestV1 =
            serde_json::from_slice(&manifest_bytes).expect("decode manifest");
        assert_eq!(manifest.chain_id, 42);
        assert_eq!(manifest.archive_start_block, 11);
        assert_eq!(manifest.latest_contiguous_uploaded_block, Some(11));

        handle.abort();
        let _ = handle.await;
    });
}

#[test]
fn failed_upload_preserves_row_and_updates_retry_metadata() {
    let _guard = ARCHIVE_TEST_LOCK.lock().expect("archive test lock");
    common::run(async {
        common::reset_archive_tables().await;

        let mut config = common::archive_config("unused-bucket");
        config.retry_base_seconds = 10;
        let uploader = ArchiveUploader::new(
            common::pool().clone(),
            Arc::new(FailingStore) as Arc<dyn ArchiveObjectStore>,
            config,
            42,
        );

        seed_archive_block(common::pool(), "integration", 42, 21, 21).await;

        let started_at = Utc::now();
        let handle = tokio::spawn(async move { uploader.run().await });

        wait_until(Duration::from_secs(5), || {
            Box::pin(async {
                let row: (
                    Option<DateTime<Utc>>,
                    Option<Vec<u8>>,
                    i32,
                    Option<String>,
                    Option<DateTime<Utc>>,
                ) = sqlx::query_as(
                    "SELECT uploaded_at, payload, retry_count, last_error, next_attempt_at
                     FROM archive_blocks WHERE block_number = 21",
                )
                .fetch_one(common::pool())
                .await
                .expect("fetch failed archive row");
                row.0.is_none()
                    && row.1.is_some()
                    && row.2 == 1
                    && row
                        .3
                        .as_deref()
                        .is_some_and(|err| err.contains("simulated archive upload failure"))
                    && row.4.is_some()
            })
        })
        .await;

        let row: (i32, DateTime<Utc>) = sqlx::query_as(
            "SELECT retry_count, next_attempt_at
             FROM archive_blocks WHERE block_number = 21",
        )
        .fetch_one(common::pool())
        .await
        .expect("fetch retry timing");
        assert_eq!(row.0, 1);
        assert!(
            row.1 >= started_at + chrono::Duration::seconds(9),
            "next retry should use the configured base delay"
        );
        assert!(
            row.1 <= started_at + chrono::Duration::seconds(15),
            "next retry should not skip directly to the doubled delay"
        );

        handle.abort();
        let _ = handle.await;
    });
}

#[test]
fn late_lower_block_rebases_archive_start_and_recovers_contiguous_head() {
    let _guard = ARCHIVE_TEST_LOCK.lock().expect("archive test lock");
    common::run(async {
        common::reset_archive_tables().await;

        let config: ArchiveConfig = common::archive_config("unused-bucket");
        let prefix = config.prefix.clone();
        let store = Arc::new(RecordingStore::default());

        let (mut client, connection) = tokio_postgres::connect(common::database_url(), tokio_postgres::NoTls)
            .await
            .expect("connect tokio-postgres");
        tokio::spawn(async move {
            let _ = connection.await;
        });

        let mut tx = client.transaction().await.expect("begin tx for block 11");
        insert_archive_entries(&mut tx, vec![archive_entry(&prefix, 7, 11)])
            .await
            .expect("insert block 11 archive entry");
        tx.commit().await.expect("commit block 11 archive entry");

        sqlx::query(
            "UPDATE archive_blocks
             SET uploaded_at = NOW(),
                 payload = NULL,
                 etag = 'etag-11',
                 updated_at = NOW()
             WHERE block_number = 11",
        )
        .execute(common::pool())
        .await
        .expect("mark block 11 uploaded");
        sqlx::query(
            "UPDATE archive_state
             SET latest_contiguous_uploaded_block = 11,
                 manifest_dirty = FALSE,
                 manifest_updated_at = NOW(),
                 updated_at = NOW()
             WHERE stream = 'blocks'",
        )
        .execute(common::pool())
        .await
        .expect("seed archive state");

        let mut tx = client.transaction().await.expect("begin tx for block 10");
        insert_archive_entries(&mut tx, vec![archive_entry(&prefix, 7, 10)])
            .await
            .expect("insert lower block archive entry");
        tx.commit().await.expect("commit block 10 archive entry");

        let state_after_insert: (i64, Option<i64>, bool) = sqlx::query_as(
            "SELECT archive_start_block, latest_contiguous_uploaded_block, manifest_dirty
             FROM archive_state WHERE stream = 'blocks'",
        )
        .fetch_one(common::pool())
        .await
        .expect("fetch archive state after rebasing");
        assert_eq!(state_after_insert.0, 10);
        assert_eq!(state_after_insert.1, None);
        assert!(!state_after_insert.2);

        let uploader = ArchiveUploader::new(
            common::pool().clone(),
            store.clone() as Arc<dyn ArchiveObjectStore>,
            config,
            7,
        );
        let handle = tokio::spawn(async move { uploader.run().await });

        wait_until(Duration::from_secs(5), || {
            Box::pin(async {
                let state: (i64, Option<i64>, bool, Option<DateTime<Utc>>) = sqlx::query_as(
                    "SELECT archive_start_block, latest_contiguous_uploaded_block, manifest_dirty, manifest_updated_at
                     FROM archive_state WHERE stream = 'blocks'",
                )
                .fetch_one(common::pool())
                .await
                .expect("fetch archive state");
                state.0 == 10 && state.1 == Some(11) && !state.2 && state.3.is_some()
            })
        })
        .await;

        let manifests = store.manifests.lock().expect("lock manifests");
        let last: ArchiveManifestV1 =
            serde_json::from_slice(manifests.last().expect("final manifest"))
                .expect("decode final manifest");
        assert_eq!(last.archive_start_block, 10);
        assert_eq!(last.latest_contiguous_uploaded_block, Some(11));

        handle.abort();
        let _ = handle.await;
    });
}

#[test]
fn out_of_order_uploads_do_not_advance_manifest_past_gap() {
    let _guard = ARCHIVE_TEST_LOCK.lock().expect("archive test lock");
    common::run(async {
        common::reset_archive_tables().await;

        let mut config: ArchiveConfig = common::archive_config("unused-bucket");
        config.upload_concurrency = 3;
        config.retry_base_seconds = 1;
        let store = Arc::new(FlakyStore::default());
        let uploader = ArchiveUploader::new(
            common::pool().clone(),
            store.clone() as Arc<dyn ArchiveObjectStore>,
            config,
            7,
        );

        for block_number in [30_i64, 31, 32] {
            seed_archive_block(common::pool(), "integration", 7, block_number, 30).await;
        }

        let handle = tokio::spawn(async move { uploader.run().await });

        wait_until(Duration::from_secs(5), || {
            Box::pin(async {
                let head: Option<i64> = sqlx::query_scalar(
                    "SELECT latest_contiguous_uploaded_block
                     FROM archive_state WHERE stream = 'blocks'",
                )
                .fetch_one(common::pool())
                .await
                .expect("fetch contiguous head");
                head == Some(30)
            })
        })
        .await;

        wait_until(Duration::from_secs(10), || {
            Box::pin(async {
                let state: (Option<i64>, bool, Option<DateTime<Utc>>) = sqlx::query_as(
                    "SELECT latest_contiguous_uploaded_block, manifest_dirty, manifest_updated_at
                     FROM archive_state WHERE stream = 'blocks'",
                )
                .fetch_one(common::pool())
                .await
                .expect("fetch archive state");
                state.0 == Some(32) && !state.1 && state.2.is_some()
            })
        })
        .await;

        let manifests = store.manifests.lock().expect("lock manifests");
        assert!(
            manifests.len() >= 2,
            "expected at least two manifest uploads, got {}",
            manifests.len()
        );
        let first: ArchiveManifestV1 =
            serde_json::from_slice(&manifests[0]).expect("decode first manifest");
        let last: ArchiveManifestV1 =
            serde_json::from_slice(manifests.last().expect("final manifest"))
                .expect("decode final manifest");
        assert_eq!(first.latest_contiguous_uploaded_block, Some(30));
        assert_eq!(last.latest_contiguous_uploaded_block, Some(32));

        handle.abort();
        let _ = handle.await;
    });
}
