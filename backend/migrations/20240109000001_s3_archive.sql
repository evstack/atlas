-- Archive tracking for canonical block bundles uploaded to S3-compatible storage.

CREATE TABLE IF NOT EXISTS archive_blocks (
    block_number BIGINT PRIMARY KEY,
    object_key TEXT NOT NULL,
    payload BYTEA,
    schema_version SMALLINT NOT NULL,
    retry_count INT NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_error TEXT,
    uploaded_at TIMESTAMPTZ,
    etag TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_archive_blocks_pending
    ON archive_blocks (next_attempt_at, block_number)
    WHERE uploaded_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_archive_blocks_uploaded
    ON archive_blocks (uploaded_at)
    WHERE uploaded_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS archive_state (
    stream TEXT PRIMARY KEY,
    archive_start_block BIGINT NOT NULL,
    latest_contiguous_uploaded_block BIGINT,
    schema_version SMALLINT NOT NULL,
    manifest_dirty BOOLEAN NOT NULL DEFAULT TRUE,
    last_manifest_error TEXT,
    manifest_updated_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
