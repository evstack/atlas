-- Add metadata_fetched flag to NFT and ERC-20 contracts tables

ALTER TABLE nft_contracts ADD COLUMN IF NOT EXISTS metadata_fetched BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE erc20_contracts ADD COLUMN IF NOT EXISTS metadata_fetched BOOLEAN NOT NULL DEFAULT false;

-- Index for efficient lookup of contracts needing metadata
CREATE INDEX IF NOT EXISTS idx_nft_contracts_metadata_pending
    ON nft_contracts(metadata_fetched) WHERE metadata_fetched = false;
CREATE INDEX IF NOT EXISTS idx_erc20_contracts_metadata_pending
    ON erc20_contracts(metadata_fetched) WHERE metadata_fetched = false;
