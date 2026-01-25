-- Atlas Contract Verification Migration
-- Extends contract_abis table with verification-specific fields

-- Add new columns to contract_abis for full verification support
ALTER TABLE contract_abis
    ADD COLUMN IF NOT EXISTS contract_name VARCHAR(255),
    ADD COLUMN IF NOT EXISTS constructor_args BYTEA,
    ADD COLUMN IF NOT EXISTS evm_version VARCHAR(32),
    ADD COLUMN IF NOT EXISTS license_type VARCHAR(64),
    ADD COLUMN IF NOT EXISTS is_multi_file BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS source_files JSONB; -- For multi-file contracts: {"file.sol": "source..."}

-- Index for looking up verified contracts
CREATE INDEX IF NOT EXISTS idx_contract_abis_verified ON contract_abis(verified_at);
CREATE INDEX IF NOT EXISTS idx_contract_abis_compiler ON contract_abis(compiler_version);
