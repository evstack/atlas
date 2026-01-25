use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

use atlas_common::{
    AtlasError, Erc20Balance, Erc20Contract, Erc20Holder, Erc20Transfer, Pagination,
    PaginatedResponse,
};
use crate::AppState;
use crate::error::ApiResult;

/// GET /api/tokens - List all ERC-20 tokens
pub async fn list_tokens(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Erc20Contract>>> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM erc20_contracts")
        .fetch_one(&state.pool)
        .await?;

    let tokens: Vec<Erc20Contract> = sqlx::query_as(
        "SELECT address, name, symbol, decimals, total_supply, first_seen_block
         FROM erc20_contracts
         ORDER BY first_seen_block DESC
         LIMIT $1 OFFSET $2",
    )
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

/// Token detail response with holder count
#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenDetailResponse {
    #[serde(flatten)]
    pub contract: Erc20Contract,
    pub holder_count: i64,
    pub transfer_count: i64,
}

/// GET /api/tokens/:address - Get token details
pub async fn get_token(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<TokenDetailResponse>> {
    let address = normalize_address(&address);

    let contract: Erc20Contract = sqlx::query_as(
        "SELECT address, name, symbol, decimals, total_supply, first_seen_block
         FROM erc20_contracts
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Token {} not found", address)))?;

    let holder_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM erc20_balances
         WHERE LOWER(contract_address) = LOWER($1) AND balance > 0",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    let transfer_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM erc20_transfers WHERE LOWER(contract_address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(TokenDetailResponse {
        contract,
        holder_count: holder_count.0,
        transfer_count: transfer_count.0,
    }))
}

/// GET /api/tokens/:address/holders - Get token holders
pub async fn get_token_holders(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Erc20Holder>>> {
    let address = normalize_address(&address);

    // Verify token exists
    let _: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM erc20_contracts WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AtlasError::NotFound(format!("Token {} not found", address)))?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM erc20_balances
         WHERE LOWER(contract_address) = LOWER($1) AND balance > 0",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    // Get total supply for percentage calculation
    let total_supply: Option<(bigdecimal::BigDecimal,)> = sqlx::query_as(
        "SELECT total_supply FROM erc20_contracts WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    let balances: Vec<Erc20Balance> = sqlx::query_as(
        "SELECT address, contract_address, balance, last_updated_block
         FROM erc20_balances
         WHERE LOWER(contract_address) = LOWER($1) AND balance > 0
         ORDER BY balance DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    // Convert to Erc20Holder with percentage
    let holders: Vec<Erc20Holder> = balances
        .into_iter()
        .map(|b| {
            let percentage = total_supply
                .as_ref()
                .and_then(|(ts,)| {
                    use bigdecimal::ToPrimitive;
                    let balance_f = b.balance.to_f64()?;
                    let supply_f = ts.to_f64()?;
                    if supply_f > 0.0 {
                        Some((balance_f / supply_f) * 100.0)
                    } else {
                        None
                    }
                });
            Erc20Holder {
                address: b.address,
                balance: b.balance,
                percentage,
            }
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        holders,
        pagination.page,
        pagination.limit,
        total.0,
    )))
}

/// GET /api/tokens/:address/transfers - Get token transfers
pub async fn get_token_transfers(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Erc20Transfer>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM erc20_transfers WHERE LOWER(contract_address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    let transfers: Vec<Erc20Transfer> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, contract_address, from_address, to_address, value, block_number, timestamp
         FROM erc20_transfers
         WHERE LOWER(contract_address) = LOWER($1)
         ORDER BY block_number DESC, log_index DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        transfers,
        pagination.page,
        pagination.limit,
        total.0,
    )))
}

/// GET /api/addresses/:address/tokens - Get ERC-20 balances for address
pub async fn get_address_tokens(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<AddressTokenBalance>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM erc20_balances
         WHERE LOWER(address) = LOWER($1) AND balance > 0",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    let balances: Vec<AddressTokenBalance> = sqlx::query_as(
        "SELECT b.address, b.contract_address, b.balance, b.last_updated_block,
                c.name, c.symbol, c.decimals
         FROM erc20_balances b
         JOIN erc20_contracts c ON LOWER(b.contract_address) = LOWER(c.address)
         WHERE LOWER(b.address) = LOWER($1) AND b.balance > 0
         ORDER BY b.balance DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        balances,
        pagination.page,
        pagination.limit,
        total.0,
    )))
}

/// Token balance with contract info for address endpoint
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct AddressTokenBalance {
    pub address: String,
    pub contract_address: String,
    pub balance: bigdecimal::BigDecimal,
    pub last_updated_block: i64,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: i16,
}

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}
