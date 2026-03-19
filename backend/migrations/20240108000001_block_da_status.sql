-- Block DA (Data Availability) status for L2 chains using Celestia.
-- Only populated when ENABLE_DA_TRACKING=true and the DA worker is running.
--
-- The DA worker has two phases:
-- 1. Backfill: discovers blocks missing from this table, queries ev-node, and INSERTs.
--    Always inserts a row even when DA heights are 0 (not yet included on Celestia).
--    This marks the block as "checked" so backfill won't re-query it.
-- 2. Update pending: retries rows where header_da_height = 0 OR data_da_height = 0
--    until real DA heights are returned by ev-node.

CREATE TABLE IF NOT EXISTS block_da_status (
    block_number BIGINT PRIMARY KEY,
    header_da_height BIGINT NOT NULL DEFAULT 0,
    data_da_height BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Partial index for the DA worker to efficiently find blocks still pending DA inclusion.
CREATE INDEX IF NOT EXISTS idx_block_da_status_pending
    ON block_da_status (block_number)
    WHERE header_da_height = 0 OR data_da_height = 0;
