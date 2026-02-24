use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

use crate::error::ApiResult;
use crate::handlers::get_table_count;
use crate::AppState;
use atlas_common::{
    AtlasError, Erc20Transfer, NftTransfer, PaginatedResponse, Pagination, Transaction,
};

pub async fn list_transactions(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Transaction>>> {
    // Use optimized count (approximate for large tables, exact for small)
    let total = get_table_count(&state.pool).await?;

    let transactions: Vec<Transaction> = sqlx::query_as(
        "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
         FROM transactions
         ORDER BY block_number DESC, block_index DESC
         LIMIT $1 OFFSET $2"
    )
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        transactions,
        pagination.page,
        pagination.limit,
        total,
    )))
}

pub async fn get_transaction(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> ApiResult<Json<Transaction>> {
    let hash = normalize_hash(&hash);

    let transaction: Transaction = sqlx::query_as(
        "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
         FROM transactions
         WHERE hash = $1"
    )
    .bind(&hash)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Transaction {} not found", hash)))?;

    Ok(Json(transaction))
}

/// GET /api/transactions/{hash}/erc20-transfers - Get all ERC-20 transfers in a transaction
pub async fn get_transaction_erc20_transfers(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Erc20Transfer>>> {
    let hash = normalize_hash(&hash);

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM erc20_transfers WHERE tx_hash = $1")
        .bind(&hash)
        .fetch_one(&state.pool)
        .await?;

    let transfers: Vec<Erc20Transfer> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, contract_address, from_address, to_address, value, block_number, timestamp
         FROM erc20_transfers
         WHERE tx_hash = $1
         ORDER BY log_index ASC
         LIMIT $2 OFFSET $3"
    )
    .bind(&hash)
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

/// GET /api/transactions/{hash}/nft-transfers - Get all NFT transfers in a transaction
pub async fn get_transaction_nft_transfers(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<NftTransfer>>> {
    let hash = normalize_hash(&hash);

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM nft_transfers WHERE tx_hash = $1")
        .bind(&hash)
        .fetch_one(&state.pool)
        .await?;

    let transfers: Vec<NftTransfer> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, contract_address, token_id, from_address, to_address, block_number, timestamp
         FROM nft_transfers
         WHERE tx_hash = $1
         ORDER BY log_index ASC
         LIMIT $2 OFFSET $3"
    )
    .bind(&hash)
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

fn normalize_hash(hash: &str) -> String {
    if hash.starts_with("0x") {
        hash.to_lowercase()
    } else {
        format!("0x{}", hash.to_lowercase())
    }
}
