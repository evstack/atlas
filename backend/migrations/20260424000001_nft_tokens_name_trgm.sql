CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX IF NOT EXISTS idx_nft_tokens_name_trgm ON nft_tokens USING GIN (name gin_trgm_ops) WHERE name IS NOT NULL;
