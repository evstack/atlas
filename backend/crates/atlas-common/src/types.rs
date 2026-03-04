use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Block data as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Block {
    pub number: i64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: i64,
    pub gas_used: i64,
    pub gas_limit: i64,
    pub transaction_count: i32,
    pub indexed_at: DateTime<Utc>,
}

/// Transaction data as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transaction {
    pub hash: String,
    pub block_number: i64,
    pub block_index: i32,
    pub from_address: String,
    pub to_address: Option<String>,
    pub value: BigDecimal,
    pub gas_price: BigDecimal,
    pub gas_used: i64,
    pub input_data: Vec<u8>,
    pub status: bool,
    pub contract_created: Option<String>,
    pub timestamp: i64,
}

/// Address data as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Address {
    pub address: String,
    pub is_contract: bool,
    pub first_seen_block: i64,
    pub tx_count: i32,
}

/// NFT Contract (ERC-721) as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NftContract {
    pub address: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub total_supply: Option<i64>,
    pub first_seen_block: i64,
}

/// NFT Token as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NftToken {
    pub contract_address: String,
    pub token_id: BigDecimal,
    pub owner: String,
    pub token_uri: Option<String>,
    pub metadata_fetched: bool,
    pub metadata: Option<serde_json::Value>,
    pub image_url: Option<String>,
    pub name: Option<String>,
    pub last_transfer_block: i64,
}

/// NFT Transfer event as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NftTransfer {
    pub id: i64,
    pub tx_hash: String,
    pub log_index: i32,
    pub contract_address: String,
    pub token_id: BigDecimal,
    pub from_address: String,
    pub to_address: String,
    pub block_number: i64,
    pub timestamp: i64,
}

/// Indexer state tracking
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IndexerState {
    pub key: String,
    pub value: String,
    pub updated_at: DateTime<Utc>,
}

// =====================
// ERC-20 Token Types
// =====================

/// ERC-20 Contract as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Erc20Contract {
    pub address: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: i16,
    pub total_supply: Option<BigDecimal>,
    pub first_seen_block: i64,
}

/// ERC-20 Transfer event as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Erc20Transfer {
    pub id: i64,
    pub tx_hash: String,
    pub log_index: i32,
    pub contract_address: String,
    pub from_address: String,
    pub to_address: String,
    pub value: BigDecimal,
    pub block_number: i64,
    pub timestamp: i64,
}

/// ERC-20 Balance as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Erc20Balance {
    pub address: String,
    pub contract_address: String,
    pub balance: BigDecimal,
    pub last_updated_block: i64,
}

/// ERC-20 holder with balance for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Erc20Holder {
    pub address: String,
    pub balance: BigDecimal,
    pub percentage: Option<f64>,
}

// =====================
// Event Log Types
// =====================

/// Event log as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EventLog {
    pub id: i64,
    pub tx_hash: String,
    pub log_index: i32,
    pub address: String,
    pub topic0: String,
    pub topic1: Option<String>,
    pub topic2: Option<String>,
    pub topic3: Option<String>,
    pub data: Vec<u8>,
    pub block_number: i64,
    pub decoded: Option<serde_json::Value>,
}

/// Known event signature for decoding
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EventSignature {
    pub signature: String,
    pub name: String,
    pub full_signature: String,
    pub abi: Option<serde_json::Value>,
}

// =====================
// Address Label Types
// =====================

/// Address label as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AddressLabel {
    pub address: String,
    pub name: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Address label for creation/update (without timestamps)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressLabelInput {
    pub address: String,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

// =====================
// Proxy Contract Types
// =====================

/// Proxy contract relationship as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProxyContract {
    pub proxy_address: String,
    pub implementation_address: String,
    pub proxy_type: String,
    pub admin_address: Option<String>,
    pub detected_at_block: i64,
    pub last_checked_block: i64,
    pub updated_at: DateTime<Utc>,
}

/// Proxy type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    /// EIP-1967 Transparent Proxy
    Eip1967,
    /// EIP-1822 UUPS Proxy
    Eip1822,
    /// OpenZeppelin Transparent Proxy
    Transparent,
    /// Custom/Unknown proxy pattern
    Custom,
}

impl ProxyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProxyType::Eip1967 => "eip1967",
            ProxyType::Eip1822 => "eip1822",
            ProxyType::Transparent => "transparent",
            ProxyType::Custom => "custom",
        }
    }
}

impl std::fmt::Display for ProxyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =====================
// Contract ABI Types
// =====================

/// Contract ABI as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContractAbi {
    pub address: String,
    pub abi: serde_json::Value,
    pub source_code: Option<String>,
    pub compiler_version: Option<String>,
    pub optimization_used: Option<bool>,
    pub runs: Option<i32>,
    pub verified_at: DateTime<Utc>,
}

// =====================
// Contract Verification Types
// =====================

/// Extended verified contract data including all verification fields
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VerifiedContract {
    pub address: String,
    pub abi: serde_json::Value,
    pub source_code: Option<String>,
    pub compiler_version: Option<String>,
    pub optimization_used: Option<bool>,
    pub runs: Option<i32>,
    pub verified_at: DateTime<Utc>,
    pub contract_name: Option<String>,
    pub constructor_args: Option<Vec<u8>>,
    pub evm_version: Option<String>,
    pub license_type: Option<String>,
    pub is_multi_file: bool,
    pub source_files: Option<serde_json::Value>,
}

/// Request to verify a contract
#[derive(Debug, Clone, Deserialize)]
pub struct VerifyContractRequest {
    /// Contract address to verify
    pub address: String,
    /// Solidity source code (single file) or standard JSON input
    pub source_code: String,
    /// Contract name (e.g., "MyContract" or "path/to/File.sol:ContractName")
    pub contract_name: String,
    /// Compiler version (e.g., "0.8.20", "v0.8.20+commit.a1b2c3d4")
    pub compiler_version: String,
    /// Whether optimization was enabled
    #[serde(default)]
    pub optimization_enabled: bool,
    /// Number of optimization runs (default: 200)
    #[serde(default = "default_optimization_runs")]
    pub optimization_runs: u32,
    /// Constructor arguments (hex encoded, without 0x prefix)
    #[serde(default)]
    pub constructor_args: Option<String>,
    /// EVM version (e.g., "paris", "shanghai")
    #[serde(default)]
    pub evm_version: Option<String>,
    /// License type (e.g., "MIT", "GPL-3.0")
    #[serde(default)]
    pub license_type: Option<String>,
    /// Whether source is standard JSON input format
    #[serde(default)]
    pub is_standard_json: bool,
}

fn default_optimization_runs() -> u32 {
    200
}

/// Response from contract verification
#[derive(Debug, Clone, Serialize)]
pub struct VerifyContractResponse {
    pub success: bool,
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abi: Option<serde_json::Value>,
}

/// Verification error details
#[derive(Debug, Clone, Serialize)]
pub struct VerificationError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Source code response
#[derive(Debug, Clone, Serialize)]
pub struct ContractSourceResponse {
    pub address: String,
    pub contract_name: Option<String>,
    pub source_code: String,
    pub compiler_version: Option<String>,
    pub optimization_enabled: bool,
    pub optimization_runs: i32,
    pub evm_version: Option<String>,
    pub license_type: Option<String>,
    pub constructor_args: Option<String>,
    pub is_multi_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_files: Option<serde_json::Value>,
    pub verified_at: DateTime<Utc>,
}

/// ABI response
#[derive(Debug, Clone, Serialize)]
pub struct ContractAbiResponse {
    pub address: String,
    pub abi: serde_json::Value,
    pub contract_name: Option<String>,
    pub verified_at: DateTime<Utc>,
}

/// Pagination parameters
#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    20
}

impl Pagination {
    pub fn offset(&self) -> i64 {
        ((self.page.saturating_sub(1)) * self.limit) as i64
    }

    pub fn limit(&self) -> i64 {
        self.limit.min(100) as i64
    }
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub page: u32,
    pub limit: u32,
    pub total: i64,
    pub total_pages: u32,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, page: u32, limit: u32, total: i64) -> Self {
        let total_pages = ((total as f64) / (limit as f64)).ceil() as u32;
        Self {
            data,
            page,
            limit,
            total,
            total_pages,
        }
    }
}
