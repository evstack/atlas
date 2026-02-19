-- Search Performance Indexes
-- Optimizes the /api/search endpoint

-- =====================
-- Trigram Index for NFT Name Search
-- =====================
-- Enable pg_trgm extension for fuzzy/ILIKE searches
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Replace btree index with trigram GIN index for fast ILIKE searches
DROP INDEX IF EXISTS idx_nft_tokens_name;
CREATE INDEX IF NOT EXISTS idx_nft_tokens_name_trgm ON nft_tokens USING GIN (name gin_trgm_ops);

-- =====================
-- Transaction Hash Lookup
-- =====================
-- Since transactions are partitioned by block_number, searching by hash alone
-- requires scanning all partitions. This lookup table provides O(1) access.
CREATE TABLE IF NOT EXISTS tx_hash_lookup (
    hash VARCHAR(66) PRIMARY KEY,
    block_number BIGINT NOT NULL
);

-- Populate from existing transactions (run once)
INSERT INTO tx_hash_lookup (hash, block_number)
SELECT hash, block_number FROM transactions
ON CONFLICT (hash) DO NOTHING;

-- =====================
-- Token Contract Name Search
-- =====================
-- Trigram index for ERC-20 token name/symbol search
CREATE INDEX IF NOT EXISTS idx_erc20_contracts_name_trgm ON erc20_contracts USING GIN (name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_erc20_contracts_symbol_trgm ON erc20_contracts USING GIN (symbol gin_trgm_ops);

-- Trigram index for NFT collection name/symbol search
CREATE INDEX IF NOT EXISTS idx_nft_contracts_name_trgm ON nft_contracts USING GIN (name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_nft_contracts_symbol_trgm ON nft_contracts USING GIN (symbol gin_trgm_ops);
