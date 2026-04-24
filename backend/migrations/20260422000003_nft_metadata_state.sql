ALTER TABLE nft_tokens
    ADD COLUMN metadata_status TEXT NOT NULL DEFAULT 'pending',
    ADD COLUMN metadata_retry_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN next_retry_at TIMESTAMPTZ,
    ADD COLUMN last_metadata_error TEXT,
    ADD COLUMN last_metadata_attempted_at TIMESTAMPTZ,
    ADD COLUMN metadata_updated_at TIMESTAMPTZ;

ALTER TABLE nft_tokens
    ADD CONSTRAINT nft_tokens_metadata_status_check
    CHECK (
        metadata_status IN ('pending', 'fetched', 'retryable_error', 'permanent_error')
    );

UPDATE nft_tokens
SET metadata_status = CASE
        WHEN metadata IS NOT NULL OR image_url IS NOT NULL THEN 'fetched'
        ELSE 'pending'
    END,
    metadata_retry_count = 0,
    next_retry_at = CASE
        WHEN metadata IS NOT NULL OR image_url IS NOT NULL THEN NULL
        ELSE NOW()
    END,
    metadata_updated_at = CASE
        WHEN metadata IS NOT NULL OR image_url IS NOT NULL THEN NOW()
        ELSE NULL
    END;

DROP INDEX IF EXISTS idx_nft_tokens_metadata_pending;

CREATE INDEX IF NOT EXISTS idx_nft_tokens_metadata_queue
    ON nft_tokens (next_retry_at, last_transfer_block)
    WHERE metadata_status IN ('pending', 'retryable_error');

ALTER TABLE nft_tokens
    DROP COLUMN metadata_fetched;
