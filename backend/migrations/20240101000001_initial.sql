-- Atlas Blockchain Explorer Schema (with Partitioning)
-- Partitioned by block_number for scalability

-- ============================================
-- BLOCKS (Partitioned by block number)
-- ============================================
-- Partition size: 10 million blocks per partition
-- Partitions are auto-created by the indexer

CREATE TABLE IF NOT EXISTS blocks (
    number BIGINT NOT NULL,
    hash VARCHAR(66) NOT NULL,
    parent_hash VARCHAR(66) NOT NULL,
    timestamp BIGINT NOT NULL,
    gas_used BIGINT NOT NULL,
    gas_limit BIGINT NOT NULL,
    transaction_count INTEGER NOT NULL,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (number)
) PARTITION BY RANGE (number);

-- Create initial partition (0 to 10M)
-- Additional partitions are created automatically by the indexer
CREATE TABLE IF NOT EXISTS blocks_p0 PARTITION OF blocks
    FOR VALUES FROM (0) TO (10000000);

CREATE INDEX IF NOT EXISTS idx_blocks_hash ON blocks(hash);
CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blocks(timestamp);

-- ============================================
-- TRANSACTIONS (Partitioned by block number)
-- ============================================
-- No foreign key to blocks (allows independent partitioning)

CREATE TABLE IF NOT EXISTS transactions (
    hash VARCHAR(66) NOT NULL,
    block_number BIGINT NOT NULL,
    block_index INTEGER NOT NULL,
    from_address VARCHAR(42) NOT NULL,
    to_address VARCHAR(42),
    value NUMERIC(78, 0) NOT NULL,
    gas_price NUMERIC(78, 0) NOT NULL,
    gas_used BIGINT NOT NULL,
    input_data BYTEA NOT NULL,
    status BOOLEAN NOT NULL,
    contract_created VARCHAR(42),
    timestamp BIGINT NOT NULL,
    PRIMARY KEY (hash, block_number)
) PARTITION BY RANGE (block_number);

-- Create initial partition
CREATE TABLE IF NOT EXISTS transactions_p0 PARTITION OF transactions
    FOR VALUES FROM (0) TO (10000000);

CREATE INDEX IF NOT EXISTS idx_transactions_block ON transactions(block_number);
CREATE INDEX IF NOT EXISTS idx_transactions_from ON transactions(from_address);
CREATE INDEX IF NOT EXISTS idx_transactions_to ON transactions(to_address);
CREATE INDEX IF NOT EXISTS idx_transactions_timestamp ON transactions(timestamp);

-- ============================================
-- EVENT LOGS (Partitioned by block number)
-- ============================================

CREATE TABLE IF NOT EXISTS event_logs (
    id BIGSERIAL,
    tx_hash VARCHAR(66) NOT NULL,
    log_index INTEGER NOT NULL,
    address VARCHAR(42) NOT NULL,
    topic0 VARCHAR(66) NOT NULL,
    topic1 VARCHAR(66),
    topic2 VARCHAR(66),
    topic3 VARCHAR(66),
    data BYTEA NOT NULL,
    block_number BIGINT NOT NULL,
    decoded JSONB,
    PRIMARY KEY (id, block_number),
    UNIQUE (tx_hash, log_index, block_number)
) PARTITION BY RANGE (block_number);

-- Create initial partition
CREATE TABLE IF NOT EXISTS event_logs_p0 PARTITION OF event_logs
    FOR VALUES FROM (0) TO (10000000);

CREATE INDEX IF NOT EXISTS idx_event_logs_address ON event_logs(address);
CREATE INDEX IF NOT EXISTS idx_event_logs_topic0 ON event_logs(topic0);
CREATE INDEX IF NOT EXISTS idx_event_logs_block ON event_logs(block_number);
CREATE INDEX IF NOT EXISTS idx_event_logs_tx ON event_logs(tx_hash);

-- ============================================
-- NON-PARTITIONED TABLES
-- ============================================

-- Addresses (not partitioned - relatively small)
CREATE TABLE IF NOT EXISTS addresses (
    address VARCHAR(42) PRIMARY KEY,
    is_contract BOOLEAN NOT NULL DEFAULT FALSE,
    first_seen_block BIGINT NOT NULL,
    tx_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_addresses_contract ON addresses(is_contract) WHERE is_contract = TRUE;

-- NFT Contracts (ERC-721)
CREATE TABLE IF NOT EXISTS nft_contracts (
    address VARCHAR(42) PRIMARY KEY,
    name VARCHAR(255),
    symbol VARCHAR(32),
    total_supply BIGINT,
    first_seen_block BIGINT NOT NULL
);

-- NFT Tokens
CREATE TABLE IF NOT EXISTS nft_tokens (
    contract_address VARCHAR(42) NOT NULL REFERENCES nft_contracts(address) ON DELETE CASCADE,
    token_id NUMERIC(78, 0) NOT NULL,
    owner VARCHAR(42) NOT NULL,
    token_uri TEXT,
    metadata_fetched BOOLEAN NOT NULL DEFAULT FALSE,
    metadata JSONB,
    image_url TEXT,
    name VARCHAR(255),
    last_transfer_block BIGINT NOT NULL,
    PRIMARY KEY (contract_address, token_id)
);

CREATE INDEX IF NOT EXISTS idx_nft_tokens_owner ON nft_tokens(owner);
CREATE INDEX IF NOT EXISTS idx_nft_tokens_name ON nft_tokens(name) WHERE name IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_nft_tokens_metadata_pending ON nft_tokens(metadata_fetched) WHERE metadata_fetched = FALSE;

-- NFT Transfers (Partitioned by block number)
CREATE TABLE IF NOT EXISTS nft_transfers (
    id BIGSERIAL,
    tx_hash VARCHAR(66) NOT NULL,
    log_index INTEGER NOT NULL,
    contract_address VARCHAR(42) NOT NULL,
    token_id NUMERIC(78, 0) NOT NULL,
    from_address VARCHAR(42) NOT NULL,
    to_address VARCHAR(42) NOT NULL,
    block_number BIGINT NOT NULL,
    timestamp BIGINT NOT NULL,
    PRIMARY KEY (id, block_number)
) PARTITION BY RANGE (block_number);

-- Create initial partition
CREATE TABLE IF NOT EXISTS nft_transfers_p0 PARTITION OF nft_transfers
    FOR VALUES FROM (0) TO (10000000);

CREATE INDEX IF NOT EXISTS idx_nft_transfers_token ON nft_transfers(contract_address, token_id);
CREATE INDEX IF NOT EXISTS idx_nft_transfers_from ON nft_transfers(from_address);
CREATE INDEX IF NOT EXISTS idx_nft_transfers_to ON nft_transfers(to_address);
CREATE INDEX IF NOT EXISTS idx_nft_transfers_block ON nft_transfers(block_number);

-- Indexer State
CREATE TABLE IF NOT EXISTS indexer_state (
    key VARCHAR(64) PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Full-text search index for NFT metadata
CREATE INDEX IF NOT EXISTS idx_nft_tokens_metadata_fts ON nft_tokens USING GIN (metadata jsonb_path_ops);
