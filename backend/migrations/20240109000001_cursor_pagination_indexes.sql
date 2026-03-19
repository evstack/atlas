-- Composite index for cursor-based (keyset) pagination on transactions.
-- Enables O(log N) index seeks instead of O(N) OFFSET scans for the
-- global transaction listing endpoint.
CREATE INDEX IF NOT EXISTS idx_transactions_block_idx
ON transactions(block_number DESC, block_index DESC);
