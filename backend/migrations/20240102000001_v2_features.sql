-- Atlas v2 Features Migration
-- ERC-20 Token Support, Event Signatures, Address Labels, Proxy Detection

-- =====================
-- ERC-20 Token Support
-- =====================

-- ERC-20 Contracts (not partitioned - relatively small)
CREATE TABLE IF NOT EXISTS erc20_contracts (
    address VARCHAR(42) PRIMARY KEY,
    name VARCHAR(255),
    symbol VARCHAR(32),
    decimals SMALLINT NOT NULL DEFAULT 18,
    total_supply NUMERIC(78, 0),
    first_seen_block BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_erc20_contracts_symbol ON erc20_contracts(symbol);

-- ERC-20 Transfers (Partitioned by block number)
CREATE TABLE IF NOT EXISTS erc20_transfers (
    id BIGSERIAL,
    tx_hash VARCHAR(66) NOT NULL,
    log_index INTEGER NOT NULL,
    contract_address VARCHAR(42) NOT NULL,
    from_address VARCHAR(42) NOT NULL,
    to_address VARCHAR(42) NOT NULL,
    value NUMERIC(78, 0) NOT NULL,
    block_number BIGINT NOT NULL,
    timestamp BIGINT NOT NULL,
    PRIMARY KEY (id, block_number),
    UNIQUE (tx_hash, log_index, block_number)
) PARTITION BY RANGE (block_number);

-- Create initial partition
CREATE TABLE IF NOT EXISTS erc20_transfers_p0 PARTITION OF erc20_transfers
    FOR VALUES FROM (0) TO (10000000);

CREATE INDEX IF NOT EXISTS idx_erc20_transfers_contract ON erc20_transfers(contract_address);
CREATE INDEX IF NOT EXISTS idx_erc20_transfers_from ON erc20_transfers(from_address);
CREATE INDEX IF NOT EXISTS idx_erc20_transfers_to ON erc20_transfers(to_address);
CREATE INDEX IF NOT EXISTS idx_erc20_transfers_block ON erc20_transfers(block_number);

-- ERC-20 Balances (not partitioned - updated incrementally)
CREATE TABLE IF NOT EXISTS erc20_balances (
    address VARCHAR(42) NOT NULL,
    contract_address VARCHAR(42) NOT NULL REFERENCES erc20_contracts(address) ON DELETE CASCADE,
    balance NUMERIC(78, 0) NOT NULL DEFAULT 0,
    last_updated_block BIGINT NOT NULL,
    PRIMARY KEY (address, contract_address)
);

CREATE INDEX IF NOT EXISTS idx_erc20_balances_contract ON erc20_balances(contract_address);
CREATE INDEX IF NOT EXISTS idx_erc20_balances_holder ON erc20_balances(address);

-- =====================
-- Event Signatures
-- =====================

-- Known event signatures for decoding
CREATE TABLE IF NOT EXISTS event_signatures (
    signature VARCHAR(66) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    full_signature TEXT NOT NULL,
    abi JSONB
);

-- Insert common event signatures
INSERT INTO event_signatures (signature, name, full_signature) VALUES
    ('0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef', 'Transfer', 'Transfer(address,address,uint256)'),
    ('0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925', 'Approval', 'Approval(address,address,uint256)'),
    ('0x17307eab39ab6107e8899845ad3d59bd9653f200f220920489ca2b5937696c31', 'ApprovalForAll', 'ApprovalForAll(address,address,bool)'),
    ('0xe1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c', 'Deposit', 'Deposit(address,uint256)'),
    ('0x7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65', 'Withdrawal', 'Withdrawal(address,uint256)'),
    ('0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822', 'Swap', 'Swap(address,uint256,uint256,uint256,uint256,address)')
ON CONFLICT (signature) DO NOTHING;

-- =====================
-- Address Labels
-- =====================

CREATE TABLE IF NOT EXISTS address_labels (
    address VARCHAR(42) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    tags TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_address_labels_tags ON address_labels USING GIN(tags);
CREATE INDEX IF NOT EXISTS idx_address_labels_name ON address_labels(name);

-- =====================
-- Proxy Contract Detection
-- =====================

CREATE TABLE IF NOT EXISTS proxy_contracts (
    proxy_address VARCHAR(42) PRIMARY KEY,
    implementation_address VARCHAR(42) NOT NULL,
    proxy_type VARCHAR(32) NOT NULL, -- 'eip1967', 'eip1822', 'transparent', 'custom'
    admin_address VARCHAR(42),
    detected_at_block BIGINT NOT NULL,
    last_checked_block BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_proxy_contracts_impl ON proxy_contracts(implementation_address);
CREATE INDEX IF NOT EXISTS idx_proxy_contracts_admin ON proxy_contracts(admin_address) WHERE admin_address IS NOT NULL;

-- =====================
-- Contract ABIs (for decoding)
-- =====================

CREATE TABLE IF NOT EXISTS contract_abis (
    address VARCHAR(42) PRIMARY KEY,
    abi JSONB NOT NULL,
    source_code TEXT,
    compiler_version VARCHAR(64),
    optimization_used BOOLEAN,
    runs INTEGER,
    verified_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
