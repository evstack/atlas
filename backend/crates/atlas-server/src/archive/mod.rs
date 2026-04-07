use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client as S3Client;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tokio_postgres::{types::ToSql, Transaction};

use crate::config::ArchiveConfig;
use crate::indexer::fetcher::FetchedBlock;

pub const ARCHIVE_SCHEMA_VERSION: i16 = 1;
const CLAIM_VISIBILITY_TIMEOUT_SECS: i64 = 300;
const IDLE_SLEEP: Duration = Duration::from_millis(500);
const MAX_RETRY_DELAY_SECS: u64 = 3600;
const ARCHIVE_STREAM_BLOCKS: &str = "blocks";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveBundleV1 {
    pub schema_version: i16,
    pub chain_id: u64,
    pub block_number: u64,
    pub block: alloy::rpc::types::Block,
    pub receipts: Vec<alloy::rpc::types::TransactionReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveManifestV1 {
    pub schema_version: i16,
    pub chain_id: u64,
    pub archive_start_block: i64,
    pub latest_contiguous_uploaded_block: Option<i64>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub block_number: i64,
    pub object_key: String,
    pub payload: Vec<u8>,
    pub schema_version: i16,
}

#[derive(Debug, Clone, FromRow)]
struct PendingArchiveRow {
    block_number: i64,
    object_key: String,
    payload: Option<Vec<u8>>,
    retry_count: i32,
}

#[derive(Debug, Clone, FromRow)]
struct ArchiveStateRow {
    archive_start_block: i64,
    latest_contiguous_uploaded_block: Option<i64>,
    schema_version: i16,
    manifest_dirty: bool,
}

#[async_trait]
pub trait ArchiveObjectStore: Send + Sync {
    async fn ensure_bucket_access(&self) -> Result<()>;
    async fn put_archive_object(&self, key: &str, payload: Vec<u8>) -> Result<Option<String>>;
    async fn put_manifest_object(&self, key: &str, payload: Vec<u8>) -> Result<Option<String>>;
}

#[derive(Clone)]
pub struct S3ArchiveStore {
    client: S3Client,
    bucket: String,
}

impl S3ArchiveStore {
    pub fn new(client: S3Client, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }

    pub async fn from_config(config: &ArchiveConfig) -> Result<Self> {
        let shared_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .load()
            .await;
        let mut builder = aws_sdk_s3::config::Builder::from(&shared_config)
            .force_path_style(config.force_path_style);
        if let Some(endpoint) = &config.endpoint {
            builder = builder.endpoint_url(endpoint);
        }

        Ok(Self {
            client: S3Client::from_conf(builder.build()),
            bucket: config.bucket.clone(),
        })
    }
}

#[async_trait]
impl ArchiveObjectStore for S3ArchiveStore {
    async fn ensure_bucket_access(&self) -> Result<()> {
        self.client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .context("failed to access archive bucket")?;
        Ok(())
    }

    async fn put_archive_object(&self, key: &str, payload: Vec<u8>) -> Result<Option<String>> {
        let response = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type("application/zstd")
            .body(ByteStream::from(payload))
            .send()
            .await
            .with_context(|| format!("failed to upload archive object {key}"))?;
        Ok(response.e_tag().map(ToOwned::to_owned))
    }

    async fn put_manifest_object(&self, key: &str, payload: Vec<u8>) -> Result<Option<String>> {
        let response = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type("application/json")
            .body(ByteStream::from(payload))
            .send()
            .await
            .with_context(|| format!("failed to upload archive manifest {key}"))?;
        Ok(response.e_tag().map(ToOwned::to_owned))
    }
}

#[derive(Clone)]
pub struct ArchiveUploader {
    pool: PgPool,
    store: Arc<dyn ArchiveObjectStore>,
    config: ArchiveConfig,
    chain_id: u64,
}

impl ArchiveUploader {
    pub fn new(
        pool: PgPool,
        store: Arc<dyn ArchiveObjectStore>,
        config: ArchiveConfig,
        chain_id: u64,
    ) -> Self {
        Self {
            pool,
            store,
            config,
            chain_id,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let mut join_set = tokio::task::JoinSet::new();
        let workers = self.config.upload_concurrency.max(1);

        for _ in 0..workers {
            let uploader = self.clone();
            join_set.spawn(async move { uploader.worker_loop().await });
        }

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(err)) => return Err(err),
                Err(err) => return Err(anyhow!("archive worker task failed: {err}")),
            }
        }

        Ok(())
    }

    async fn worker_loop(&self) -> Result<()> {
        loop {
            let mut did_work = false;

            if let Some(row) = self.claim_pending_row().await? {
                did_work = true;
                self.process_row(row).await?;
            }

            if self.try_upload_manifest().await? {
                did_work = true;
            }

            if !did_work {
                tokio::time::sleep(IDLE_SLEEP).await;
            }
        }
    }

    async fn claim_pending_row(&self) -> Result<Option<PendingArchiveRow>> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_as::<_, PendingArchiveRow>(
            "WITH next AS (
                 SELECT block_number
                 FROM archive_blocks
                 WHERE uploaded_at IS NULL
                   AND payload IS NOT NULL
                   AND next_attempt_at <= NOW()
                 ORDER BY next_attempt_at ASC, block_number ASC
                 LIMIT 1
                 FOR UPDATE SKIP LOCKED
             )
             UPDATE archive_blocks AS blocks
             SET next_attempt_at = NOW() + make_interval(secs => $1),
                 updated_at = NOW()
             FROM next
             WHERE blocks.block_number = next.block_number
             RETURNING blocks.block_number, blocks.object_key, blocks.payload, blocks.retry_count",
        )
        .bind(CLAIM_VISIBILITY_TIMEOUT_SECS)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row)
    }

    async fn process_row(&self, row: PendingArchiveRow) -> Result<()> {
        let payload = row
            .payload
            .ok_or_else(|| anyhow!("archive row {} is missing payload", row.block_number))?;

        match self
            .store
            .put_archive_object(&row.object_key, payload)
            .await
        {
            Ok(etag) => {
                let mut tx = self.pool.begin().await?;
                sqlx::query(
                    "UPDATE archive_blocks
                     SET uploaded_at = NOW(),
                         etag = $2,
                         payload = NULL,
                         last_error = NULL,
                         updated_at = NOW()
                     WHERE block_number = $1",
                )
                .bind(row.block_number)
                .bind(etag)
                .execute(&mut *tx)
                .await?;

                if self.advance_contiguous_head(&mut tx).await? {
                    sqlx::query(
                        "UPDATE archive_state
                         SET manifest_dirty = TRUE,
                             updated_at = NOW()
                         WHERE stream = $1",
                    )
                    .bind(ARCHIVE_STREAM_BLOCKS)
                    .execute(&mut *tx)
                    .await?;
                }

                tx.commit().await?;
                Ok(())
            }
            Err(err) => {
                let next_retry_count = row.retry_count + 1;
                let delay = retry_delay_seconds(self.config.retry_base_seconds, row.retry_count);
                if delay >= MAX_RETRY_DELAY_SECS {
                    tracing::warn!(
                        block_number = row.block_number,
                        retry_count = next_retry_count,
                        "archive upload retry delay has hit the 1-hour cap; block is stuck"
                    );
                }
                let next_attempt_at = Utc::now() + chrono::Duration::seconds(delay as i64);
                sqlx::query(
                    "UPDATE archive_blocks
                     SET retry_count = retry_count + 1,
                         last_error = $2,
                         next_attempt_at = $3,
                         updated_at = NOW()
                     WHERE block_number = $1",
                )
                .bind(row.block_number)
                .bind(err.to_string())
                .bind(next_attempt_at)
                .execute(&self.pool)
                .await?;
                Ok(())
            }
        }
    }

    async fn advance_contiguous_head(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<bool> {
        let state = sqlx::query_as::<_, ArchiveStateRow>(
            "SELECT archive_start_block, latest_contiguous_uploaded_block, schema_version, manifest_dirty
             FROM archive_state
             WHERE stream = $1
             FOR UPDATE",
        )
        .bind(ARCHIVE_STREAM_BLOCKS)
        .fetch_optional(&mut **tx)
        .await?;

        let Some(state) = state else {
            return Ok(false);
        };

        let mut expected = state
            .latest_contiguous_uploaded_block
            .map(|n| n + 1)
            .unwrap_or(state.archive_start_block);
        let mut latest = state.latest_contiguous_uploaded_block;

        loop {
            let rows: Vec<(i64,)> = sqlx::query_as(
                "SELECT block_number
                 FROM archive_blocks
                 WHERE uploaded_at IS NOT NULL
                   AND block_number >= $1
                 ORDER BY block_number ASC
                 LIMIT 512",
            )
            .bind(expected)
            .fetch_all(&mut **tx)
            .await?;

            if rows.is_empty() {
                break;
            }

            let mut advanced = false;
            for (block_number,) in rows {
                if block_number != expected {
                    break;
                }
                latest = Some(block_number);
                expected += 1;
                advanced = true;
            }

            if !advanced {
                break;
            }
        }

        if latest != state.latest_contiguous_uploaded_block {
            sqlx::query(
                "UPDATE archive_state
                 SET latest_contiguous_uploaded_block = $2,
                     updated_at = NOW()
                 WHERE stream = $1",
            )
            .bind(ARCHIVE_STREAM_BLOCKS)
            .bind(latest)
            .execute(&mut **tx)
            .await?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn try_upload_manifest(&self) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let state = sqlx::query_as::<_, ArchiveStateRow>(
            "SELECT archive_start_block, latest_contiguous_uploaded_block, schema_version, manifest_dirty
             FROM archive_state
             WHERE stream = $1
             FOR UPDATE",
        )
        .bind(ARCHIVE_STREAM_BLOCKS)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(state) = state else {
            tx.commit().await?;
            return Ok(false);
        };

        if !state.manifest_dirty {
            tx.commit().await?;
            return Ok(false);
        }

        let now = Utc::now();
        let manifest = ArchiveManifestV1 {
            schema_version: state.schema_version,
            chain_id: self.chain_id,
            archive_start_block: state.archive_start_block,
            latest_contiguous_uploaded_block: state.latest_contiguous_uploaded_block,
            updated_at: now.to_rfc3339(),
        };
        let payload = serde_json::to_vec(&manifest)?;

        match self
            .store
            .put_manifest_object(&manifest_object_key(&self.config.prefix), payload)
            .await
        {
            Ok(_) => {
                sqlx::query(
                    "UPDATE archive_state
                     SET manifest_dirty = FALSE,
                         manifest_updated_at = $2,
                         last_manifest_error = NULL,
                         updated_at = $2
                     WHERE stream = $1",
                )
                .bind(ARCHIVE_STREAM_BLOCKS)
                .bind(now)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok(true)
            }
            Err(err) => {
                sqlx::query(
                    "UPDATE archive_state
                     SET last_manifest_error = $2,
                         updated_at = NOW()
                     WHERE stream = $1",
                )
                .bind(ARCHIVE_STREAM_BLOCKS)
                .bind(err.to_string())
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok(false)
            }
        }
    }
}

impl ArchiveEntry {
    pub(crate) fn from_fetched_block(
        chain_id: u64,
        prefix: &str,
        fetched: &FetchedBlock,
    ) -> Result<Self> {
        let bundle = ArchiveBundleV1 {
            schema_version: ARCHIVE_SCHEMA_VERSION,
            chain_id,
            block_number: fetched.number,
            block: fetched.block.clone(),
            receipts: fetched.receipts.clone(),
        };
        let payload = zstd::stream::encode_all(serde_json::to_vec(&bundle)?.as_slice(), 3)?;
        Ok(Self {
            block_number: fetched.number as i64,
            object_key: block_object_key(prefix, fetched.number),
            payload,
            schema_version: ARCHIVE_SCHEMA_VERSION,
        })
    }
}

pub async fn insert_archive_entries(
    tx: &mut Transaction<'_>,
    entries: Vec<ArchiveEntry>,
) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut block_numbers = Vec::with_capacity(entries.len());
    let mut object_keys = Vec::with_capacity(entries.len());
    let mut payloads = Vec::with_capacity(entries.len());
    let mut schema_versions = Vec::with_capacity(entries.len());

    for entry in entries {
        block_numbers.push(entry.block_number);
        object_keys.push(entry.object_key);
        payloads.push(entry.payload);
        schema_versions.push(entry.schema_version);
    }

    let min_block = *block_numbers
        .iter()
        .min()
        .ok_or_else(|| anyhow!("archive entries unexpectedly empty"))?;

    let init_params: [&(dyn ToSql + Sync); 3] =
        [&ARCHIVE_STREAM_BLOCKS, &min_block, &ARCHIVE_SCHEMA_VERSION];
    tx.execute(
        "INSERT INTO archive_state
            (stream, archive_start_block, latest_contiguous_uploaded_block, schema_version, manifest_dirty)
         VALUES ($1, $2, NULL, $3, FALSE)
         ON CONFLICT (stream) DO UPDATE SET
            archive_start_block = LEAST(archive_state.archive_start_block, EXCLUDED.archive_start_block),
            latest_contiguous_uploaded_block = CASE
                WHEN EXCLUDED.archive_start_block < archive_state.archive_start_block THEN NULL
                ELSE archive_state.latest_contiguous_uploaded_block
            END,
            schema_version = GREATEST(archive_state.schema_version, EXCLUDED.schema_version),
            manifest_dirty = archive_state.manifest_dirty,
            updated_at = NOW()",
        &init_params,
    )
    .await?;

    let params: [&(dyn ToSql + Sync); 4] =
        [&block_numbers, &object_keys, &payloads, &schema_versions];
    tx.execute(
        "INSERT INTO archive_blocks
            (block_number, object_key, payload, schema_version, retry_count, next_attempt_at, created_at, updated_at)
         SELECT block_number, object_key, payload, schema_version, 0, NOW(), NOW(), NOW()
         FROM unnest($1::bigint[], $2::text[], $3::bytea[], $4::smallint[])
            AS t(block_number, object_key, payload, schema_version)
         ON CONFLICT (block_number) DO UPDATE SET
            object_key = EXCLUDED.object_key,
            payload = EXCLUDED.payload,
            schema_version = EXCLUDED.schema_version,
            retry_count = 0,
            next_attempt_at = NOW(),
            last_error = NULL,
            uploaded_at = NULL,
            etag = NULL,
            updated_at = NOW()",
        &params,
    )
    .await?;

    Ok(())
}

pub fn block_object_key(prefix: &str, block_number: u64) -> String {
    let bucket_start = (block_number / 10_000) * 10_000;
    if prefix.is_empty() {
        format!("v1/blocks/{bucket_start:012}/{block_number:012}.json.zst")
    } else {
        format!("{prefix}/v1/blocks/{bucket_start:012}/{block_number:012}.json.zst")
    }
}

pub fn manifest_object_key(prefix: &str) -> String {
    if prefix.is_empty() {
        "v1/manifest.json".to_string()
    } else {
        format!("{prefix}/v1/manifest.json")
    }
}

pub fn retry_delay_seconds(base: u64, retry_count: i32) -> u64 {
    let exponent = retry_count.max(0).min(16) as u32;
    base.saturating_mul(2u64.saturating_pow(exponent))
        .min(MAX_RETRY_DELAY_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_object_key_uses_expected_layout() {
        assert_eq!(
            block_object_key("atlas/dev", 10_237),
            "atlas/dev/v1/blocks/000000010000/000000010237.json.zst"
        );
    }

    #[test]
    fn manifest_key_respects_prefix() {
        assert_eq!(manifest_object_key(""), "v1/manifest.json");
        assert_eq!(
            manifest_object_key("atlas/dev"),
            "atlas/dev/v1/manifest.json"
        );
    }

    #[test]
    fn retry_delay_is_exponential_and_capped() {
        assert_eq!(retry_delay_seconds(30, 0), 30);
        assert_eq!(retry_delay_seconds(30, 1), 60);
        assert_eq!(retry_delay_seconds(30, 2), 120);
        assert_eq!(retry_delay_seconds(30, 10), 3600);
    }

    #[test]
    fn archive_manifest_round_trip() {
        let manifest = ArchiveManifestV1 {
            schema_version: ARCHIVE_SCHEMA_VERSION,
            chain_id: 42,
            archive_start_block: 100,
            latest_contiguous_uploaded_block: Some(150),
            updated_at: Utc::now().to_rfc3339(),
        };
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let decoded: ArchiveManifestV1 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.schema_version, ARCHIVE_SCHEMA_VERSION);
        assert_eq!(decoded.chain_id, 42);
        assert_eq!(decoded.archive_start_block, 100);
        assert_eq!(decoded.latest_contiguous_uploaded_block, Some(150));
    }

    #[test]
    fn archive_bundle_round_trip_through_zstd() {
        let bundle = ArchiveBundleV1 {
            schema_version: ARCHIVE_SCHEMA_VERSION,
            chain_id: 42,
            block_number: 7,
            block: alloy::rpc::types::Block::default(),
            receipts: Vec::new(),
        };
        let compressed =
            zstd::stream::encode_all(serde_json::to_vec(&bundle).unwrap().as_slice(), 3).unwrap();
        let decoded = zstd::stream::decode_all(compressed.as_slice()).unwrap();
        let round_trip: ArchiveBundleV1 = serde_json::from_slice(&decoded).unwrap();

        assert_eq!(round_trip.schema_version, ARCHIVE_SCHEMA_VERSION);
        assert_eq!(round_trip.chain_id, 42);
        assert_eq!(round_trip.block_number, 7);
        assert!(round_trip.receipts.is_empty());
    }
}
