//! Etherscan-compatible API endpoints
//!
//! Implements the Etherscan API format for compatibility with tooling like Hardhat and Foundry.
//! Response format: { "status": "1", "message": "OK", "result": ... }

use alloy::providers::{Provider, ProviderBuilder};
use axum::{
    extract::{Form, Query, State},
    Json,
};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use atlas_common::{AtlasError, ContractAbi, Transaction, VerifyContractRequest};
use crate::AppState;
use crate::error::ApiResult;
use crate::handlers::contracts;

/// Etherscan API response wrapper
#[derive(Debug, Serialize)]
pub struct EtherscanResponse<T> {
    pub status: String,
    pub message: String,
    pub result: T,
}

impl<T> EtherscanResponse<T> {
    pub fn ok(result: T) -> Self {
        Self {
            status: "1".to_string(),
            message: "OK".to_string(),
            result,
        }
    }

    pub fn error(message: impl Into<String>, result: T) -> Self {
        Self {
            status: "0".to_string(),
            message: message.into(),
            result,
        }
    }
}

/// Query parameters for Etherscan API
#[derive(Debug, Deserialize)]
pub struct EtherscanQuery {
    pub module: String,
    pub action: String,
    /// Single address for account queries
    pub address: Option<String>,
    /// Contract address for token queries
    pub contractaddress: Option<String>,
    /// Transaction hash
    pub txhash: Option<String>,
    /// Block number
    pub blockno: Option<String>,
    /// Start block for range queries
    pub startblock: Option<i64>,
    /// End block for range queries
    pub endblock: Option<i64>,
    /// Page number
    pub page: Option<u32>,
    /// Results per page
    pub offset: Option<u32>,
    /// Sort order (asc/desc)
    pub sort: Option<String>,
    /// API key (optional, for rate limiting)
    pub apikey: Option<String>,
}

/// Etherscan-compatible contract verification request (form data)
#[derive(Debug, Deserialize)]
pub struct EtherscanVerifyRequest {
    pub module: String,
    pub action: String,
    /// Contract address to verify
    #[serde(rename = "contractaddress")]
    pub contract_address: String,
    /// Solidity source code
    #[serde(rename = "sourceCode")]
    pub source_code: String,
    /// Contract name (e.g., "contracts/MyContract.sol:MyContract")
    #[serde(rename = "contractname")]
    pub contract_name: String,
    /// Compiler version (e.g., "v0.8.20+commit.a1b2c3d4")
    #[serde(rename = "compilerversion")]
    pub compiler_version: String,
    /// Optimization used (0 or 1)
    #[serde(rename = "optimizationUsed", default)]
    pub optimization_used: String,
    /// Number of optimization runs
    #[serde(default = "default_runs_str")]
    pub runs: String,
    /// Constructor arguments (hex encoded)
    #[serde(rename = "constructorArguements", default)] // Note: Etherscan typo is intentional
    pub constructor_arguments: String,
    /// EVM version
    #[serde(rename = "evmversion", default)]
    pub evm_version: String,
    /// License type (1-14 mapped to SPDX identifiers)
    #[serde(rename = "licenseType", default)]
    pub license_type: String,
    /// API key
    #[serde(default)]
    pub apikey: String,
}

fn default_runs_str() -> String {
    "200".to_string()
}

/// Main Etherscan API router (GET requests)
pub async fn etherscan_api(
    State(state): State<Arc<AppState>>,
    Query(query): Query<EtherscanQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    match query.module.as_str() {
        "account" => handle_account_module(state, query).await,
        "contract" => handle_contract_module(state, query).await,
        "transaction" => handle_transaction_module(state, query).await,
        "block" => handle_block_module(state, query).await,
        "proxy" => handle_proxy_module(state, query).await,
        _ => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            format!("Unknown module: {}", query.module),
            serde_json::Value::Null,
        ))?)),
    }
}

/// Etherscan API POST handler (for verification)
pub async fn etherscan_api_post(
    State(state): State<Arc<AppState>>,
    Form(form): Form<EtherscanVerifyRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    if form.module != "contract" {
        return Ok(Json(serde_json::to_value(EtherscanResponse::error(
            format!("POST only supported for contract module, got: {}", form.module),
            serde_json::Value::Null,
        ))?));
    }

    match form.action.as_str() {
        "verifysourcecode" => verify_source_code_etherscan(state, form).await,
        _ => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            format!("Unknown action: {}", form.action),
            serde_json::Value::Null,
        ))?)),
    }
}

/// Etherscan-compatible verifysourcecode implementation
async fn verify_source_code_etherscan(
    state: Arc<AppState>,
    form: EtherscanVerifyRequest,
) -> ApiResult<Json<serde_json::Value>> {
    // Detect if source is standard JSON input
    let is_standard_json = form.source_code.trim().starts_with('{')
        && form.source_code.contains("\"language\"")
        && form.source_code.contains("\"sources\"");

    // Parse optimization setting
    let optimization_enabled = form.optimization_used == "1";

    // Parse runs
    let optimization_runs: u32 = form.runs.parse().unwrap_or(200);

    // Map license type number to SPDX identifier
    let license_type = map_license_type(&form.license_type);

    // Build the internal verification request
    let request = VerifyContractRequest {
        address: form.contract_address,
        source_code: form.source_code,
        contract_name: form.contract_name,
        compiler_version: form.compiler_version,
        optimization_enabled,
        optimization_runs,
        constructor_args: if form.constructor_arguments.is_empty() {
            None
        } else {
            Some(form.constructor_arguments)
        },
        evm_version: if form.evm_version.is_empty() {
            None
        } else {
            Some(form.evm_version)
        },
        license_type,
        is_standard_json,
    };

    // Call the internal verification logic
    match contracts::verify_contract(
        axum::extract::State(state),
        Json(request),
    ).await {
        Ok(Json(response)) => {
            if response.success {
                // Etherscan returns a GUID for async verification, we verify synchronously
                // Return success with address as the "GUID"
                Ok(Json(serde_json::to_value(EtherscanResponse::ok(
                    response.address
                ))?))
            } else {
                Ok(Json(serde_json::to_value(EtherscanResponse::error(
                    response.message.unwrap_or_else(|| "Verification failed".to_string()),
                    serde_json::Value::Null,
                ))?))
            }
        }
        Err(e) => {
            Ok(Json(serde_json::to_value(EtherscanResponse::error(
                e.to_string(),
                serde_json::Value::Null,
            ))?))
        }
    }
}

/// Map Etherscan license type numbers to SPDX identifiers
fn map_license_type(license_num: &str) -> Option<String> {
    match license_num {
        "1" => Some("Unlicense".to_string()),
        "2" => Some("MIT".to_string()),
        "3" => Some("GPL-2.0".to_string()),
        "4" => Some("GPL-3.0".to_string()),
        "5" => Some("LGPL-2.1".to_string()),
        "6" => Some("LGPL-3.0".to_string()),
        "7" => Some("BSD-2-Clause".to_string()),
        "8" => Some("BSD-3-Clause".to_string()),
        "9" => Some("MPL-2.0".to_string()),
        "10" => Some("OSL-3.0".to_string()),
        "11" => Some("Apache-2.0".to_string()),
        "12" => Some("AGPL-3.0".to_string()),
        "13" => Some("BSL-1.1".to_string()),
        "14" => Some("BUSL-1.1".to_string()),
        _ => None,
    }
}

/// Handle account module requests
async fn handle_account_module(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    match query.action.as_str() {
        "balance" => get_balance(state, query).await,
        "balancemulti" => get_balance_multi(state, query).await,
        "txlist" => get_tx_list(state, query).await,
        "txlistinternal" => get_internal_tx_list(state, query).await,
        "tokentx" => get_token_tx_list(state, query).await,
        "tokenbalance" => get_token_balance(state, query).await,
        _ => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            format!("Unknown action: {}", query.action),
            serde_json::Value::Null,
        ))?)),
    }
}

/// Handle contract module requests
async fn handle_contract_module(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    match query.action.as_str() {
        "getabi" => get_contract_abi(state, query).await,
        "getsourcecode" => get_source_code(state, query).await,
        _ => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            format!("Unknown action: {}", query.action),
            serde_json::Value::Null,
        ))?)),
    }
}

/// Handle transaction module requests
async fn handle_transaction_module(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    match query.action.as_str() {
        "gettxreceiptstatus" => get_tx_receipt_status(state, query).await,
        _ => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            format!("Unknown action: {}", query.action),
            serde_json::Value::Null,
        ))?)),
    }
}

/// Handle block module requests
async fn handle_block_module(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    match query.action.as_str() {
        "getblockreward" => get_block_reward(state, query).await,
        _ => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            format!("Unknown action: {}", query.action),
            serde_json::Value::Null,
        ))?)),
    }
}

/// Handle proxy module requests (pass-through to RPC)
async fn handle_proxy_module(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    let provider = ProviderBuilder::new()
        .on_http(state.rpc_url.parse().map_err(|e| AtlasError::Config(format!("Invalid RPC URL: {}", e)))?);

    match query.action.as_str() {
        "eth_blockNumber" => {
            let block_number = provider.get_block_number().await
                .map_err(|e| AtlasError::Rpc(e.to_string()))?;
            Ok(Json(serde_json::to_value(EtherscanResponse::ok(
                format!("0x{:x}", block_number),
            ))?))
        }
        "eth_getBlockByNumber" => {
            let block_no = query.blockno.as_ref()
                .ok_or_else(|| AtlasError::InvalidInput("blockno required".to_string()))?;
            let block_num: u64 = if let Some(stripped) = block_no.strip_prefix("0x") {
                u64::from_str_radix(stripped, 16)
                    .map_err(|_| AtlasError::InvalidInput("Invalid block number".to_string()))?
            } else {
                block_no.parse()
                    .map_err(|_| AtlasError::InvalidInput("Invalid block number".to_string()))?
            };
            let block = provider
                .get_block_by_number(alloy::rpc::types::BlockNumberOrTag::Number(block_num), alloy::rpc::types::BlockTransactionsKind::Full)
                .await
                .map_err(|e| AtlasError::Rpc(e.to_string()))?;
            Ok(Json(serde_json::to_value(EtherscanResponse::ok(block))?))
        }
        "eth_getTransactionByHash" => {
            let hash = query.txhash.as_ref()
                .ok_or_else(|| AtlasError::InvalidInput("txhash required".to_string()))?;
            let hash_bytes: alloy::primitives::B256 = hash.parse()
                .map_err(|_| AtlasError::InvalidInput("Invalid transaction hash".to_string()))?;
            let tx = provider.get_transaction_by_hash(hash_bytes).await
                .map_err(|e| AtlasError::Rpc(e.to_string()))?;
            Ok(Json(serde_json::to_value(EtherscanResponse::ok(tx))?))
        }
        _ => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            format!("Unknown proxy action: {}", query.action),
            serde_json::Value::Null,
        ))?)),
    }
}

// =====================
// Account Module Actions
// =====================

async fn get_balance(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    let address = query.address.as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("address required".to_string()))?;
    let address = normalize_address(address);

    // Get balance from RPC
    let provider = ProviderBuilder::new()
        .on_http(state.rpc_url.parse().map_err(|e| AtlasError::Config(format!("Invalid RPC URL: {}", e)))?);

    let addr: alloy::primitives::Address = address.parse()
        .map_err(|_| AtlasError::InvalidInput("Invalid address".to_string()))?;

    let balance = provider.get_balance(addr).await
        .map_err(|e| AtlasError::Rpc(e.to_string()))?;

    Ok(Json(serde_json::to_value(EtherscanResponse::ok(
        balance.to_string(),
    ))?))
}

async fn get_balance_multi(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    let addresses_str = query.address.as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("address required".to_string()))?;

    let provider = ProviderBuilder::new()
        .on_http(state.rpc_url.parse().map_err(|e| AtlasError::Config(format!("Invalid RPC URL: {}", e)))?);

    let addresses: Vec<&str> = addresses_str.split(',').collect();
    let mut results = Vec::new();

    for addr_str in addresses {
        let addr_str = normalize_address(addr_str.trim());
        let addr: alloy::primitives::Address = addr_str.parse()
            .map_err(|_| AtlasError::InvalidInput(format!("Invalid address: {}", addr_str)))?;

        let balance = provider.get_balance(addr).await
            .map_err(|e| AtlasError::Rpc(e.to_string()))?;

        results.push(serde_json::json!({
            "account": addr_str,
            "balance": balance.to_string()
        }));
    }

    Ok(Json(serde_json::to_value(EtherscanResponse::ok(results))?))
}

/// Transaction list item in Etherscan format
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EtherscanTransaction {
    block_number: String,
    time_stamp: String,
    hash: String,
    nonce: String,
    block_hash: String,
    transaction_index: String,
    from: String,
    to: String,
    value: String,
    gas: String,
    gas_price: String,
    is_error: String,
    txreceipt_status: String,
    input: String,
    contract_address: String,
    cumulative_gas_used: String,
    gas_used: String,
    confirmations: String,
}

async fn get_tx_list(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    let address = query.address.as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("address required".to_string()))?;
    let address = normalize_address(address);

    let page = query.page.unwrap_or(1);
    let limit = query.offset.unwrap_or(10).min(100) as i64;
    let offset = ((page.saturating_sub(1)) as i64) * limit;
    let sort = query.sort.as_deref().unwrap_or("desc");
    let order = if sort == "asc" { "ASC" } else { "DESC" };

    let sql = format!(
        "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
         FROM transactions
         WHERE LOWER(from_address) = LOWER($1) OR LOWER(to_address) = LOWER($1)
         ORDER BY block_number {}, block_index {}
         LIMIT $2 OFFSET $3",
        order, order
    );

    let transactions: Vec<Transaction> = sqlx::query_as(&sql)
        .bind(&address)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;

    // Get current block for confirmations
    let current_block: (i64,) = sqlx::query_as("SELECT COALESCE(MAX(number), 0) FROM blocks")
        .fetch_one(&state.pool)
        .await?;

    let result: Vec<EtherscanTransaction> = transactions
        .into_iter()
        .map(|tx| {
            let confirmations = current_block.0 - tx.block_number;
            EtherscanTransaction {
                block_number: tx.block_number.to_string(),
                time_stamp: tx.timestamp.to_string(),
                hash: tx.hash,
                nonce: "0".to_string(), // Not stored
                block_hash: "".to_string(), // Would need join
                transaction_index: tx.block_index.to_string(),
                from: tx.from_address,
                to: tx.to_address.unwrap_or_default(),
                value: tx.value.to_string(),
                gas: tx.gas_used.to_string(),
                gas_price: tx.gas_price.to_string(),
                is_error: if tx.status { "0" } else { "1" }.to_string(),
                txreceipt_status: if tx.status { "1" } else { "0" }.to_string(),
                input: format!("0x{}", hex::encode(&tx.input_data)),
                contract_address: tx.contract_created.unwrap_or_default(),
                cumulative_gas_used: "0".to_string(), // Not stored
                gas_used: tx.gas_used.to_string(),
                confirmations: confirmations.to_string(),
            }
        })
        .collect();

    Ok(Json(serde_json::to_value(EtherscanResponse::ok(result))?))
}

async fn get_internal_tx_list(
    _state: Arc<AppState>,
    _query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    // Internal transactions require trace support - return empty for now
    Ok(Json(serde_json::to_value(EtherscanResponse::ok(
        Vec::<serde_json::Value>::new(),
    ))?))
}

/// Token transfer in Etherscan format
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EtherscanTokenTransfer {
    block_number: String,
    time_stamp: String,
    hash: String,
    nonce: String,
    block_hash: String,
    from: String,
    contract_address: String,
    to: String,
    value: String,
    token_name: String,
    token_symbol: String,
    token_decimal: String,
    transaction_index: String,
    gas: String,
    gas_price: String,
    gas_used: String,
    cumulative_gas_used: String,
    input: String,
    confirmations: String,
}

/// Token transfer with contract info from JOIN
#[derive(Debug, sqlx::FromRow)]
struct TokenTransferRow {
    #[allow(dead_code)]
    id: i64,
    tx_hash: String,
    log_index: i32,
    contract_address: String,
    from_address: String,
    to_address: String,
    value: BigDecimal,
    block_number: i64,
    timestamp: i64,
    name: Option<String>,
    symbol: Option<String>,
    decimals: i16,
}

async fn get_token_tx_list(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    let address = query.address.as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("address required".to_string()))?;
    let address = normalize_address(address);

    let page = query.page.unwrap_or(1);
    let limit = query.offset.unwrap_or(10).min(100) as i64;
    let offset = ((page.saturating_sub(1)) as i64) * limit;

    let transfers: Vec<TokenTransferRow> = sqlx::query_as(
        "SELECT t.id, t.tx_hash, t.log_index, t.contract_address, t.from_address, t.to_address, t.value, t.block_number, t.timestamp,
                c.name, c.symbol, COALESCE(c.decimals, 18) as decimals
         FROM erc20_transfers t
         LEFT JOIN erc20_contracts c ON LOWER(t.contract_address) = LOWER(c.address)
         WHERE LOWER(t.from_address) = LOWER($1) OR LOWER(t.to_address) = LOWER($1)
         ORDER BY t.block_number DESC, t.log_index DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(&address)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    let current_block: (i64,) = sqlx::query_as("SELECT COALESCE(MAX(number), 0) FROM blocks")
        .fetch_one(&state.pool)
        .await?;

    let result: Vec<EtherscanTokenTransfer> = transfers
        .into_iter()
        .map(|transfer| {
            let confirmations = current_block.0 - transfer.block_number;
            EtherscanTokenTransfer {
                block_number: transfer.block_number.to_string(),
                time_stamp: transfer.timestamp.to_string(),
                hash: transfer.tx_hash,
                nonce: "0".to_string(),
                block_hash: "".to_string(),
                from: transfer.from_address,
                contract_address: transfer.contract_address,
                to: transfer.to_address,
                value: transfer.value.to_string(),
                token_name: transfer.name.unwrap_or_default(),
                token_symbol: transfer.symbol.unwrap_or_default(),
                token_decimal: transfer.decimals.to_string(),
                transaction_index: transfer.log_index.to_string(),
                gas: "0".to_string(),
                gas_price: "0".to_string(),
                gas_used: "0".to_string(),
                cumulative_gas_used: "0".to_string(),
                input: "".to_string(),
                confirmations: confirmations.to_string(),
            }
        })
        .collect();

    Ok(Json(serde_json::to_value(EtherscanResponse::ok(result))?))
}

async fn get_token_balance(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    let address = query.address.as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("address required".to_string()))?;
    let contract_address = query.contractaddress.as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("contractaddress required".to_string()))?;

    let address = normalize_address(address);
    let contract_address = normalize_address(contract_address);

    let balance: Option<(BigDecimal,)> = sqlx::query_as(
        "SELECT balance FROM erc20_balances
         WHERE LOWER(address) = LOWER($1) AND LOWER(contract_address) = LOWER($2)",
    )
    .bind(&address)
    .bind(&contract_address)
    .fetch_optional(&state.pool)
    .await?;

    let balance_str = balance
        .map(|(b,)| b.to_string())
        .unwrap_or_else(|| "0".to_string());

    Ok(Json(serde_json::to_value(EtherscanResponse::ok(balance_str))?))
}

// =====================
// Contract Module Actions
// =====================

async fn get_contract_abi(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    let address = query.address.as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("address required".to_string()))?;
    let address = normalize_address(address);

    let abi: Option<ContractAbi> = sqlx::query_as(
        "SELECT address, abi, source_code, compiler_version, optimization_used, runs, verified_at
         FROM contract_abis
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    match abi {
        Some(contract_abi) => {
            let abi_str = serde_json::to_string(&contract_abi.abi)
                .map_err(|e| AtlasError::Internal(e.to_string()))?;
            Ok(Json(serde_json::to_value(EtherscanResponse::ok(abi_str))?))
        }
        None => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            "Contract source code not verified",
            "".to_string(),
        ))?)),
    }
}

/// Source code response in Etherscan format
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct SourceCodeResult {
    source_code: String,
    #[serde(rename = "ABI")]
    abi: String,
    contract_name: String,
    compiler_version: String,
    optimization_used: String,
    runs: String,
    constructor_arguments: String,
    #[serde(rename = "EVMVersion")]
    evm_version: String,
    library: String,
    license_type: String,
    proxy: String,
    implementation: String,
    swarm_source: String,
}

async fn get_source_code(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    let address = query.address.as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("address required".to_string()))?;
    let address = normalize_address(address);

    let contract: Option<ContractAbi> = sqlx::query_as(
        "SELECT address, abi, source_code, compiler_version, optimization_used, runs, verified_at
         FROM contract_abis
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    // Check if it's a proxy
    let proxy: Option<(String, String)> = sqlx::query_as(
        "SELECT proxy_type, implementation_address FROM proxy_contracts
         WHERE LOWER(proxy_address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    match contract {
        Some(c) => {
            let abi_str = serde_json::to_string(&c.abi)
                .map_err(|e| AtlasError::Internal(e.to_string()))?;
            let result = SourceCodeResult {
                source_code: c.source_code.unwrap_or_default(),
                abi: abi_str,
                contract_name: "".to_string(), // Not stored
                compiler_version: c.compiler_version.unwrap_or_default(),
                optimization_used: if c.optimization_used.unwrap_or(false) { "1" } else { "0" }.to_string(),
                runs: c.runs.unwrap_or(200).to_string(),
                constructor_arguments: "".to_string(),
                evm_version: "".to_string(),
                library: "".to_string(),
                license_type: "".to_string(),
                proxy: if proxy.is_some() { "1" } else { "0" }.to_string(),
                implementation: proxy.as_ref().map(|(_, impl_addr)| impl_addr.clone()).unwrap_or_default(),
                swarm_source: "".to_string(),
            };
            Ok(Json(serde_json::to_value(EtherscanResponse::ok(vec![result]))?))
        }
        None => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            "Contract source code not verified",
            Vec::<SourceCodeResult>::new(),
        ))?)),
    }
}

// =====================
// Transaction Module Actions
// =====================

async fn get_tx_receipt_status(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    let txhash = query.txhash.as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("txhash required".to_string()))?;
    let txhash = normalize_hash(txhash);

    let status: Option<(bool,)> = sqlx::query_as(
        "SELECT status FROM transactions WHERE LOWER(hash) = LOWER($1)",
    )
    .bind(&txhash)
    .fetch_optional(&state.pool)
    .await?;

    match status {
        Some((success,)) => Ok(Json(serde_json::to_value(EtherscanResponse::ok(
            serde_json::json!({ "status": if success { "1" } else { "0" } }),
        ))?)),
        None => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            "Transaction not found",
            serde_json::json!({ "status": "" }),
        ))?)),
    }
}

// =====================
// Block Module Actions
// =====================

async fn get_block_reward(
    state: Arc<AppState>,
    query: EtherscanQuery,
) -> ApiResult<Json<serde_json::Value>> {
    let blockno = query.blockno.as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("blockno required".to_string()))?;
    let block_number: i64 = blockno.parse()
        .map_err(|_| AtlasError::InvalidInput("Invalid block number".to_string()))?;

    let block: Option<(i64, String, i64)> = sqlx::query_as(
        "SELECT number, hash, timestamp FROM blocks WHERE number = $1",
    )
    .bind(block_number)
    .fetch_optional(&state.pool)
    .await?;

    match block {
        Some((number, _hash, timestamp)) => {
            // L2s typically don't have block rewards in the traditional sense
            Ok(Json(serde_json::to_value(EtherscanResponse::ok(serde_json::json!({
                "blockNumber": number.to_string(),
                "timeStamp": timestamp.to_string(),
                "blockMiner": "", // L2 doesn't have miners
                "blockReward": "0",
                "uncles": [],
                "uncleInclusionReward": "0"
            })))?))
        }
        None => Ok(Json(serde_json::to_value(EtherscanResponse::error(
            "Block not found",
            serde_json::Value::Null,
        ))?)),
    }
}

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}

fn normalize_hash(hash: &str) -> String {
    if hash.starts_with("0x") {
        hash.to_lowercase()
    } else {
        format!("0x{}", hash.to_lowercase())
    }
}
