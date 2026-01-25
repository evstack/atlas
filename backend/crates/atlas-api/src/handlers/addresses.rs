use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

use atlas_common::{Address, AtlasError, NftToken, Pagination, PaginatedResponse, Transaction};
use crate::AppState;
use crate::error::ApiResult;

pub async fn get_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<Address>> {
    let address = normalize_address(&address);

    let addr: Address = sqlx::query_as(
        "SELECT address, is_contract, first_seen_block, tx_count
         FROM addresses
         WHERE LOWER(address) = LOWER($1)"
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Address {} not found", address)))?;

    Ok(Json(addr))
}

pub async fn get_address_transactions(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Transaction>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE LOWER(from_address) = LOWER($1) OR LOWER(to_address) = LOWER($1)"
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    let transactions: Vec<Transaction> = sqlx::query_as(
        "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
         FROM transactions
         WHERE LOWER(from_address) = LOWER($1) OR LOWER(to_address) = LOWER($1)
         ORDER BY block_number DESC, block_index DESC
         LIMIT $2 OFFSET $3"
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(transactions, pagination.page, pagination.limit, total.0)))
}

pub async fn get_address_nfts(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<NftToken>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM nft_tokens WHERE LOWER(owner) = LOWER($1)"
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    let tokens: Vec<NftToken> = sqlx::query_as(
        "SELECT contract_address, token_id, owner, token_uri, metadata_fetched, metadata, image_url, name, last_transfer_block
         FROM nft_tokens
         WHERE LOWER(owner) = LOWER($1)
         ORDER BY last_transfer_block DESC
         LIMIT $2 OFFSET $3"
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(tokens, pagination.page, pagination.limit, total.0)))
}

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}
