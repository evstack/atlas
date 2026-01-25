use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

use atlas_common::{AtlasError, NftContract, NftToken, NftTransfer, Pagination, PaginatedResponse};
use crate::AppState;
use crate::error::ApiResult;

pub async fn list_collections(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<NftContract>>> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM nft_contracts")
        .fetch_one(&state.pool)
        .await?;

    let collections: Vec<NftContract> = sqlx::query_as(
        "SELECT address, name, symbol, total_supply, first_seen_block
         FROM nft_contracts
         ORDER BY first_seen_block DESC
         LIMIT $1 OFFSET $2"
    )
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(collections, pagination.page, pagination.limit, total.0)))
}

pub async fn get_collection(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<NftContract>> {
    let address = normalize_address(&address);

    let collection: NftContract = sqlx::query_as(
        "SELECT address, name, symbol, total_supply, first_seen_block
         FROM nft_contracts
         WHERE LOWER(address) = LOWER($1)"
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Collection {} not found", address)))?;

    Ok(Json(collection))
}

pub async fn list_collection_tokens(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<NftToken>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM nft_tokens WHERE LOWER(contract_address) = LOWER($1)"
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    let tokens: Vec<NftToken> = sqlx::query_as(
        "SELECT contract_address, token_id, owner, token_uri, metadata_fetched, metadata, image_url, name, last_transfer_block
         FROM nft_tokens
         WHERE LOWER(contract_address) = LOWER($1)
         ORDER BY token_id ASC
         LIMIT $2 OFFSET $3"
    )
    .bind(&address)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(tokens, pagination.page, pagination.limit, total.0)))
}

pub async fn get_token(
    State(state): State<Arc<AppState>>,
    Path((address, token_id)): Path<(String, String)>,
) -> ApiResult<Json<NftToken>> {
    let address = normalize_address(&address);

    let token: NftToken = sqlx::query_as(
        "SELECT contract_address, token_id, owner, token_uri, metadata_fetched, metadata, image_url, name, last_transfer_block
         FROM nft_tokens
         WHERE LOWER(contract_address) = LOWER($1) AND token_id = $2::numeric"
    )
    .bind(&address)
    .bind(&token_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Token {}:{} not found", address, token_id)))?;

    Ok(Json(token))
}

pub async fn get_token_transfers(
    State(state): State<Arc<AppState>>,
    Path((address, token_id)): Path<(String, String)>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<NftTransfer>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM nft_transfers WHERE LOWER(contract_address) = LOWER($1) AND token_id = $2::numeric"
    )
    .bind(&address)
    .bind(&token_id)
    .fetch_one(&state.pool)
    .await?;

    let transfers: Vec<NftTransfer> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, contract_address, token_id, from_address, to_address, block_number, timestamp
         FROM nft_transfers
         WHERE LOWER(contract_address) = LOWER($1) AND token_id = $2::numeric
         ORDER BY block_number DESC, log_index DESC
         LIMIT $3 OFFSET $4"
    )
    .bind(&address)
    .bind(&token_id)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(transfers, pagination.page, pagination.limit, total.0)))
}

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}
