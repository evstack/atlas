-- Add unique constraint on nft_transfers to support idempotent inserts.
-- Mirrors the existing UNIQUE (tx_hash, log_index, block_number) on erc20_transfers.
-- The partition key (block_number) must be included for partitioned tables.
ALTER TABLE nft_transfers
    ADD CONSTRAINT nft_transfers_tx_log_block_unique
    UNIQUE (tx_hash, log_index, block_number);
