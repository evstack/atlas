use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::ApiResult;
use crate::AppState;
use atlas_common::{Address, AtlasError, NftToken, PaginatedResponse, Pagination, Transaction};

/// Merged address response that combines data from addresses, nft_contracts, and erc20_contracts tables
#[derive(Debug, Clone, Serialize)]
pub struct AddressDetailResponse {
    pub address: String,
    pub first_seen_block: i64,
    pub tx_count: i32,
    /// Address type: "eoa", "contract", "nft", "erc20"
    pub address_type: String,
    /// Token/contract name (for NFT or ERC-20 contracts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Token symbol (for NFT or ERC-20 contracts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Token decimals (for ERC-20 contracts only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<i16>,
    /// Total supply (for NFT or ERC-20 contracts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_supply: Option<String>,
}

/// Address list item with address type info
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AddressListItem {
    pub address: String,
    pub first_seen_block: i64,
    pub tx_count: i32,
    pub address_type: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddressFilters {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub is_contract: Option<bool>,
    #[serde(default)]
    pub from_block: Option<i64>,
    #[serde(default)]
    pub to_block: Option<i64>,
    /// Filter by address type: "eoa", "contract", "erc20", "nft"
    #[serde(default)]
    pub address_type: Option<String>,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    20
}

pub async fn list_addresses(
    State(state): State<Arc<AppState>>,
    Query(filters): Query<AddressFilters>,
) -> ApiResult<Json<PaginatedResponse<AddressListItem>>> {
    let page = filters.page;
    let limit = filters.limit.min(100);
    let offset = (page.saturating_sub(1) * limit) as i64;

    // Build the combined query using UNION ALL
    // We use a CTE to combine all sources and then filter/paginate
    let base_query = r#"
        WITH all_addresses AS (
            -- Regular addresses (EOAs and contracts not in token tables)
            SELECT
                a.address,
                a.is_contract,
                a.first_seen_block,
                a.tx_count,
                CASE
                    WHEN e.address IS NOT NULL THEN 'erc20'
                    WHEN n.address IS NOT NULL THEN 'nft'
                    WHEN a.is_contract THEN 'contract'
                    ELSE 'eoa'
                END as address_type,
                COALESCE(e.name, n.name) as name,
                COALESCE(e.symbol, n.symbol) as symbol
            FROM addresses a
            LEFT JOIN erc20_contracts e ON LOWER(a.address) = LOWER(e.address)
            LEFT JOIN nft_contracts n ON LOWER(a.address) = LOWER(n.address)

            UNION ALL

            -- ERC-20 contracts not in addresses table
            SELECT
                e.address,
                true as is_contract,
                e.first_seen_block,
                0 as tx_count,
                'erc20' as address_type,
                e.name,
                e.symbol
            FROM erc20_contracts e
            WHERE NOT EXISTS (SELECT 1 FROM addresses a WHERE LOWER(a.address) = LOWER(e.address))

            UNION ALL

            -- NFT contracts not in addresses table
            SELECT
                n.address,
                true as is_contract,
                n.first_seen_block,
                0 as tx_count,
                'nft' as address_type,
                n.name,
                n.symbol
            FROM nft_contracts n
            WHERE NOT EXISTS (SELECT 1 FROM addresses a WHERE LOWER(a.address) = LOWER(n.address))
        )
        SELECT * FROM all_addresses
    "#;

    // Build WHERE conditions (validated; numeric/boolean to avoid injection)
    let mut conditions: Vec<String> = Vec::new();

    if let Some(is_contract) = filters.is_contract {
        conditions.push(format!("is_contract = {}", is_contract));
    }

    if let Some(from_block) = filters.from_block {
        conditions.push(format!("first_seen_block >= {}", from_block));
    }

    if let Some(to_block) = filters.to_block {
        conditions.push(format!("first_seen_block <= {}", to_block));
    }

    if let Some(ref address_type) = filters.address_type {
        // Whitelist allowed values to prevent injection
        let at = match address_type.to_lowercase().as_str() {
            "eoa" | "contract" | "erc20" | "nft" => Some(address_type.to_lowercase()),
            _ => None,
        };
        if let Some(at) = at {
            conditions.push(format!("address_type = '{}'", at));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // Count total
    let count_query = format!(
        "{} {} ",
        base_query.replace(
            "SELECT * FROM all_addresses",
            "SELECT COUNT(*) FROM all_addresses"
        ),
        where_clause
    );
    let total: (i64,) = sqlx::query_as(&count_query).fetch_one(&state.pool).await?;

    // Fetch addresses sorted by tx_count (most active first), then by first_seen_block
    let query = format!(
        "{} {}
         ORDER BY tx_count DESC, first_seen_block DESC
         LIMIT {} OFFSET {}",
        base_query, where_clause, limit, offset
    );

    let addresses: Vec<AddressListItem> = sqlx::query_as(&query).fetch_all(&state.pool).await?;

    Ok(Json(PaginatedResponse::new(
        addresses, page, limit, total.0,
    )))
}

pub async fn get_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<AddressDetailResponse>> {
    let address = normalize_address(&address);

    // Check addresses table first
    let base_addr: Option<Address> = sqlx::query_as(
        "SELECT address, is_contract, first_seen_block, tx_count
         FROM addresses
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    // Check if it's an NFT contract
    let nft_contract: Option<NftContractRow> = sqlx::query_as(
        "SELECT address, name, symbol, total_supply, first_seen_block
         FROM nft_contracts
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    // Check if it's an ERC-20 contract
    let erc20_contract: Option<Erc20ContractRow> = sqlx::query_as(
        "SELECT address, name, symbol, decimals, total_supply, first_seen_block
         FROM erc20_contracts
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    // Merge the data
    match (base_addr, nft_contract, erc20_contract) {
        // Found in addresses table and is an NFT contract
        (Some(addr), Some(nft), None) => Ok(Json(AddressDetailResponse {
            address: addr.address,
            first_seen_block: addr.first_seen_block,
            tx_count: addr.tx_count,
            address_type: "nft".to_string(),
            name: nft.name,
            symbol: nft.symbol,
            decimals: None,
            total_supply: nft.total_supply.map(|s| s.to_string()),
        })),
        // Found in addresses table and is an ERC-20 contract
        (Some(addr), None, Some(erc20)) => Ok(Json(AddressDetailResponse {
            address: addr.address,
            first_seen_block: addr.first_seen_block,
            tx_count: addr.tx_count,
            address_type: "erc20".to_string(),
            name: erc20.name,
            symbol: erc20.symbol,
            decimals: Some(erc20.decimals),
            total_supply: erc20.total_supply.map(|s| s.to_string()),
        })),
        // Found only in addresses table (regular address or contract)
        (Some(addr), None, None) => Ok(Json(AddressDetailResponse {
            address: addr.address,
            first_seen_block: addr.first_seen_block,
            tx_count: addr.tx_count,
            address_type: if addr.is_contract { "contract" } else { "eoa" }.to_string(),
            name: None,
            symbol: None,
            decimals: None,
            total_supply: None,
        })),
        // Found only in NFT contracts table (not in addresses)
        (None, Some(nft), None) => Ok(Json(AddressDetailResponse {
            address: nft.address,
            first_seen_block: nft.first_seen_block,
            tx_count: 0,
            address_type: "nft".to_string(),
            name: nft.name,
            symbol: nft.symbol,
            decimals: None,
            total_supply: nft.total_supply.map(|s| s.to_string()),
        })),
        // Found only in ERC-20 contracts table (not in addresses)
        (None, None, Some(erc20)) => Ok(Json(AddressDetailResponse {
            address: erc20.address,
            first_seen_block: erc20.first_seen_block,
            tx_count: 0,
            address_type: "erc20".to_string(),
            name: erc20.name,
            symbol: erc20.symbol,
            decimals: Some(erc20.decimals),
            total_supply: erc20.total_supply.map(|s| s.to_string()),
        })),
        // Edge case: found in both NFT and ERC-20 (shouldn't happen, prefer ERC-20)
        (base, _, Some(erc20)) => Ok(Json(AddressDetailResponse {
            address: erc20.address.clone(),
            first_seen_block: base
                .as_ref()
                .map(|b| b.first_seen_block)
                .unwrap_or(erc20.first_seen_block),
            tx_count: base.as_ref().map(|b| b.tx_count).unwrap_or(0),
            address_type: "erc20".to_string(),
            name: erc20.name,
            symbol: erc20.symbol,
            decimals: Some(erc20.decimals),
            total_supply: erc20.total_supply.map(|s| s.to_string()),
        })),
        // Not found anywhere
        (None, None, None) => {
            Err(AtlasError::NotFound(format!("Address {} not found", address)).into())
        }
    }
}

/// Internal row type for NFT contracts query
#[derive(Debug, Clone, sqlx::FromRow)]
struct NftContractRow {
    address: String,
    name: Option<String>,
    symbol: Option<String>,
    total_supply: Option<i64>,
    first_seen_block: i64,
}

/// Internal row type for ERC-20 contracts query
#[derive(Debug, Clone, sqlx::FromRow)]
struct Erc20ContractRow {
    address: String,
    name: Option<String>,
    symbol: Option<String>,
    decimals: i16,
    total_supply: Option<bigdecimal::BigDecimal>,
    first_seen_block: i64,
}

pub async fn get_address_transactions(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Transaction>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE from_address = $1 OR to_address = $1",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    let transactions: Vec<Transaction> = sqlx::query_as(
        "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
         FROM transactions
         WHERE from_address = $1 OR to_address = $1
         ORDER BY block_number DESC, block_index DESC
         LIMIT $2 OFFSET $3"
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        transactions,
        pagination.page,
        pagination.limit,
        total.0,
    )))
}

pub async fn get_address_nfts(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<NftToken>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM nft_tokens WHERE owner = $1")
        .bind(&address)
        .fetch_one(&state.pool)
        .await?;

    let tokens: Vec<NftToken> = sqlx::query_as(
        "SELECT contract_address, token_id, owner, token_uri, metadata_fetched, metadata, image_url, name, last_transfer_block
         FROM nft_tokens
         WHERE owner = $1
         ORDER BY last_transfer_block DESC
         LIMIT $2 OFFSET $3"
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        tokens,
        pagination.page,
        pagination.limit,
        total.0,
    )))
}

/// Unified transfer type combining ERC-20 and NFT transfers
#[derive(Debug, Clone, Serialize)]
pub struct Transfer {
    pub tx_hash: String,
    pub log_index: i32,
    pub contract_address: String,
    pub from_address: String,
    pub to_address: String,
    /// For ERC-20: token amount. For NFT: token_id
    pub value: String,
    pub block_number: i64,
    pub timestamp: i64,
    /// "erc20" or "nft"
    pub transfer_type: String,
    /// Token/contract name (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_name: Option<String>,
    /// Token symbol (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TransferFilters {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Filter by transfer type: "erc20", "nft", or both if not specified
    #[serde(default)]
    pub transfer_type: Option<String>,
}

pub async fn get_address_transfers(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(filters): Query<TransferFilters>,
) -> ApiResult<Json<PaginatedResponse<Transfer>>> {
    let address = normalize_address(&address);
    let page = filters.page;
    let limit = filters.limit.min(100);
    let offset = (page.saturating_sub(1) * limit) as i64;

    // Build query based on filter
    let (count_query, data_query) = match filters.transfer_type.as_deref() {
        Some("erc20") => {
            let count = r#"
                SELECT COUNT(*) FROM erc20_transfers
                WHERE from_address = $1 OR to_address = $1
            "#;
            let data = r#"
                SELECT
                    t.tx_hash,
                    t.log_index,
                    t.contract_address,
                    t.from_address,
                    t.to_address,
                    t.value::text as value,
                    t.block_number,
                    t.timestamp,
                    'erc20' as transfer_type,
                    c.name as token_name,
                    c.symbol as token_symbol
                FROM erc20_transfers t
                LEFT JOIN erc20_contracts c ON t.contract_address = c.address
                WHERE t.from_address = $1 OR t.to_address = $1
                ORDER BY t.block_number DESC, t.log_index DESC
                LIMIT $2 OFFSET $3
            "#;
            (count.to_string(), data.to_string())
        }
        Some("nft") => {
            let count = r#"
                SELECT COUNT(*) FROM nft_transfers
                WHERE from_address = $1 OR to_address = $1
            "#;
            let data = r#"
                SELECT
                    t.tx_hash,
                    t.log_index,
                    t.contract_address,
                    t.from_address,
                    t.to_address,
                    t.token_id::text as value,
                    t.block_number,
                    t.timestamp,
                    'nft' as transfer_type,
                    c.name as token_name,
                    c.symbol as token_symbol
                FROM nft_transfers t
                LEFT JOIN nft_contracts c ON t.contract_address = c.address
                WHERE t.from_address = $1 OR t.to_address = $1
                ORDER BY t.block_number DESC, t.log_index DESC
                LIMIT $2 OFFSET $3
            "#;
            (count.to_string(), data.to_string())
        }
        _ => {
            // Both types - use UNION ALL
            let count = r#"
                SELECT (
                    SELECT COUNT(*) FROM erc20_transfers WHERE from_address = $1 OR to_address = $1
                ) + (
                    SELECT COUNT(*) FROM nft_transfers WHERE from_address = $1 OR to_address = $1
                )
            "#;
            let data = r#"
                SELECT * FROM (
                    SELECT
                        t.tx_hash,
                        t.log_index,
                        t.contract_address,
                        t.from_address,
                        t.to_address,
                        t.value::text as value,
                        t.block_number,
                        t.timestamp,
                        'erc20' as transfer_type,
                        c.name as token_name,
                        c.symbol as token_symbol
                    FROM erc20_transfers t
                    LEFT JOIN erc20_contracts c ON t.contract_address = c.address
                    WHERE t.from_address = $1 OR t.to_address = $1

                    UNION ALL

                    SELECT
                        t.tx_hash,
                        t.log_index,
                        t.contract_address,
                        t.from_address,
                        t.to_address,
                        t.token_id::text as value,
                        t.block_number,
                        t.timestamp,
                        'nft' as transfer_type,
                        c.name as token_name,
                        c.symbol as token_symbol
                    FROM nft_transfers t
                    LEFT JOIN nft_contracts c ON t.contract_address = c.address
                    WHERE t.from_address = $1 OR t.to_address = $1
                ) combined
                ORDER BY block_number DESC, log_index DESC
                LIMIT $2 OFFSET $3
            "#;
            (count.to_string(), data.to_string())
        }
    };

    let total: (i64,) = sqlx::query_as(&count_query)
        .bind(&address)
        .fetch_one(&state.pool)
        .await?;

    #[derive(sqlx::FromRow)]
    struct TransferRow {
        tx_hash: String,
        log_index: i32,
        contract_address: String,
        from_address: String,
        to_address: String,
        value: String,
        block_number: i64,
        timestamp: i64,
        transfer_type: String,
        token_name: Option<String>,
        token_symbol: Option<String>,
    }

    let rows: Vec<TransferRow> = sqlx::query_as(&data_query)
        .bind(&address)
        .bind(limit as i64)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;

    let transfers: Vec<Transfer> = rows
        .into_iter()
        .map(|r| Transfer {
            tx_hash: r.tx_hash,
            log_index: r.log_index,
            contract_address: r.contract_address,
            from_address: r.from_address,
            to_address: r.to_address,
            value: r.value,
            block_number: r.block_number,
            timestamp: r.timestamp,
            transfer_type: r.transfer_type,
            token_name: r.token_name,
            token_symbol: r.token_symbol,
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        transfers, page, limit, total.0,
    )))
}

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}
