ALTER TABLE event_logs
    ADD COLUMN IF NOT EXISTS decode_status TEXT NOT NULL DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS decoded_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS decode_attempted_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS decode_source TEXT;

UPDATE event_logs
SET decode_status = CASE
        WHEN decoded IS NOT NULL THEN 'decoded'
        ELSE 'pending'
    END,
    decoded_at = CASE
        WHEN decoded IS NOT NULL AND decoded_at IS NULL THEN NOW()
        ELSE decoded_at
    END
WHERE decode_status = 'pending';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'event_logs_decode_status_check'
    ) THEN
        ALTER TABLE event_logs
            ADD CONSTRAINT event_logs_decode_status_check
                CHECK (decode_status IN ('pending', 'decoded', 'no_abi', 'no_matching_event', 'decode_failed'));
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'event_logs_decode_source_check'
    ) THEN
        ALTER TABLE event_logs
            ADD CONSTRAINT event_logs_decode_source_check
                CHECK (decode_source IS NULL OR decode_source IN ('direct_abi', 'proxy_combined_abi'));
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_event_logs_address_cursor
    ON event_logs(address, block_number, log_index, tx_hash);

CREATE INDEX IF NOT EXISTS idx_event_logs_decode_pending
    ON event_logs(address, block_number, log_index, tx_hash)
    WHERE decode_status = 'pending';

CREATE TABLE IF NOT EXISTS event_log_decode_jobs (
    address VARCHAR(42) PRIMARY KEY,
    full_rescan BOOLEAN NOT NULL DEFAULT FALSE,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_attempted_at TIMESTAMPTZ,
    retry_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_event_log_decode_jobs_requested_at
    ON event_log_decode_jobs(requested_at);
