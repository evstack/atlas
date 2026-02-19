-- Failed blocks tracking table
-- Stores blocks that failed to fetch after multiple retries
-- These can be retried later by a background process

CREATE TABLE IF NOT EXISTS failed_blocks (
    block_number BIGINT PRIMARY KEY,
    error_message TEXT,
    retry_count INT DEFAULT 0,
    first_failed_at TIMESTAMPTZ DEFAULT NOW(),
    last_failed_at TIMESTAMPTZ DEFAULT NOW()
);

-- Index for finding blocks to retry
CREATE INDEX IF NOT EXISTS idx_failed_blocks_retry ON failed_blocks(retry_count, last_failed_at);
