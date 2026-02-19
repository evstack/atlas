use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

use atlas_common::{AtlasError, Block, Pagination, PaginatedResponse, Transaction};
use crate::AppState;
use crate::error::ApiResult;

pub async fn list_blocks(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Block>>> {
    // Use MAX(number) + 1 instead of COUNT(*) - blocks are sequential so this is accurate
    // This is ~6500x faster than COUNT(*) on large tables
    let total: (Option<i64>,) = sqlx::query_as("SELECT MAX(number) + 1 FROM blocks")
        .fetch_one(&state.pool)
        .await?;
    let total_count = total.0.unwrap_or(0);

    let blocks: Vec<Block> = sqlx::query_as(
        "SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at
         FROM blocks
         ORDER BY number DESC
         LIMIT $1 OFFSET $2"
    )
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(blocks, pagination.page, pagination.limit, total_count)))
}

pub async fn get_block(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i64>,
) -> ApiResult<Json<Block>> {
    let block: Block = sqlx::query_as(
        "SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at
         FROM blocks
         WHERE number = $1"
    )
    .bind(number)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Block {} not found", number)))?;

    Ok(Json(block))
}

pub async fn get_block_transactions(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i64>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Transaction>>> {
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE block_number = $1"
    )
    .bind(number)
    .fetch_one(&state.pool)
    .await?;

    let transactions: Vec<Transaction> = sqlx::query_as(
        "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
         FROM transactions
         WHERE block_number = $1
         ORDER BY block_index ASC
         LIMIT $2 OFFSET $3"
    )
    .bind(number)
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(transactions, pagination.page, pagination.limit, total.0)))
}
