use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Serialize;
use std::sync::Arc;

use crate::api::error::ApiResult;
use crate::api::AppState;
use atlas_common::{
    AtlasError, Block, BlockDaStatus, PaginatedResponse, Pagination, Transaction, BLOCK_COLUMNS,
};

/// Block response with optional DA status.
/// DA fields are always present in the JSON (null when no data),
/// so the frontend can rely on a stable schema.
#[derive(Serialize)]
pub struct BlockResponse {
    #[serde(flatten)]
    pub block: Block,
    pub da_status: Option<BlockDaStatus>,
}

pub async fn list_blocks(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<BlockResponse>>> {
    // Use MAX(number) + 1 instead of COUNT(*) - blocks are sequential so this is accurate
    // This is ~6500x faster than COUNT(*) on large tables
    let total: (Option<i64>,) = sqlx::query_as("SELECT MAX(number) + 1 FROM blocks")
        .fetch_one(&state.pool)
        .await?;
    let total_count = total.0.unwrap_or(0);

    // Convert page-based navigation to a keyset cursor using block numbers.
    // Blocks are sequential so: cursor = max_block - (page - 1) * limit
    // WHERE number <= cursor is O(log N) via primary key; OFFSET was O(N).
    let limit = pagination.limit();
    let cursor = (total_count - 1) - (pagination.page.saturating_sub(1) as i64) * limit;

    let blocks: Vec<Block> = sqlx::query_as(&format!(
        "SELECT {} FROM blocks WHERE number <= $2 ORDER BY number DESC LIMIT $1",
        BLOCK_COLUMNS
    ))
    .bind(limit)
    .bind(cursor)
    .fetch_all(&state.pool)
    .await?;

    // Batch-fetch DA status for all blocks in this page
    let block_numbers: Vec<i64> = blocks.iter().map(|b| b.number).collect();
    let da_rows: Vec<BlockDaStatus> = sqlx::query_as(
        "SELECT block_number, header_da_height, data_da_height, updated_at
         FROM block_da_status
         WHERE block_number = ANY($1)",
    )
    .bind(&block_numbers)
    .fetch_all(&state.pool)
    .await?;

    let da_map: std::collections::HashMap<i64, BlockDaStatus> =
        da_rows.into_iter().map(|d| (d.block_number, d)).collect();

    let responses: Vec<BlockResponse> = blocks
        .into_iter()
        .map(|block| {
            let da_status = da_map.get(&block.number).cloned();
            BlockResponse { block, da_status }
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        responses,
        pagination.page,
        pagination.limit,
        total_count,
    )))
}

pub async fn get_block(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i64>,
) -> ApiResult<Json<BlockResponse>> {
    let block: Block = sqlx::query_as(&format!(
        "SELECT {} FROM blocks WHERE number = $1",
        BLOCK_COLUMNS
    ))
    .bind(number)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Block {} not found", number)))?;

    let da_status: Option<BlockDaStatus> = sqlx::query_as(
        "SELECT block_number, header_da_height, data_da_height, updated_at
         FROM block_da_status
         WHERE block_number = $1",
    )
    .bind(number)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(BlockResponse { block, da_status }))
}

pub async fn get_block_transactions(
    State(state): State<Arc<AppState>>,
    Path(number): Path<i64>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<Transaction>>> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE block_number = $1")
        .bind(number)
        .fetch_one(&state.pool)
        .await?;

    let transactions: Vec<Transaction> = sqlx::query_as(
        "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
         FROM transactions
         WHERE block_number = $1
         ORDER BY block_index ASC
         LIMIT $2 OFFSET $3",
    )
    .bind(number)
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
