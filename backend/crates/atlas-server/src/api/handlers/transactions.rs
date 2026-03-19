use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::get_table_count;
use crate::api::error::ApiResult;
use crate::api::AppState;
use atlas_common::{
    AtlasError, Erc20Transfer, NftTransfer, PaginatedResponse, Pagination, Transaction,
};

/// Query parameters for the transaction list endpoint.
/// Supports cursor-based pagination via (block_number, block_index) for O(log N)
/// seeks on large partitioned tables, replacing O(N) OFFSET scans.
#[derive(Debug, Deserialize)]
pub struct TransactionListParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Fetch transactions older than this (block_number, block_index).
    pub before_block: Option<i64>,
    pub before_index: Option<i32>,
    /// Fetch transactions newer than this (block_number, block_index).
    pub after_block: Option<i64>,
    pub after_index: Option<i32>,
    /// When true, fetch the oldest page of transactions.
    #[serde(default)]
    pub last_page: bool,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    20
}

pub async fn list_transactions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TransactionListParams>,
) -> ApiResult<Json<PaginatedResponse<Transaction>>> {
    let total = get_table_count(&state.pool, "transactions").await?;
    let limit = params.limit.min(100) as i64;

    let transactions: Vec<Transaction> =
        if let (Some(bb), Some(bi)) = (params.before_block, params.before_index) {
            // Next page: transactions older than cursor
            sqlx::query_as(
                "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
                 FROM transactions
                 WHERE (block_number, block_index) < ($1, $2)
                 ORDER BY block_number DESC, block_index DESC
                 LIMIT $3",
            )
            .bind(bb)
            .bind(bi)
            .bind(limit)
            .fetch_all(&state.pool)
            .await?
        } else if let (Some(ab), Some(ai)) = (params.after_block, params.after_index) {
            // Prev page: transactions newer than cursor (fetch ASC, reverse)
            let mut txs: Vec<Transaction> = sqlx::query_as(
                "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
                 FROM transactions
                 WHERE (block_number, block_index) > ($1, $2)
                 ORDER BY block_number ASC, block_index ASC
                 LIMIT $3",
            )
            .bind(ab)
            .bind(ai)
            .bind(limit)
            .fetch_all(&state.pool)
            .await?;
            txs.reverse();
            txs
        } else if params.last_page {
            // Last page: oldest transactions (fetch ASC, reverse)
            let mut txs: Vec<Transaction> = sqlx::query_as(
                "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
                 FROM transactions
                 ORDER BY block_number ASC, block_index ASC
                 LIMIT $1",
            )
            .bind(limit)
            .fetch_all(&state.pool)
            .await?;
            txs.reverse();
            txs
        } else {
            // First page (default): newest transactions
            sqlx::query_as(
                "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
                 FROM transactions
                 ORDER BY block_number DESC, block_index DESC
                 LIMIT $1",
            )
            .bind(limit)
            .fetch_all(&state.pool)
            .await?
        };

    Ok(Json(PaginatedResponse::new(
        transactions,
        params.page,
        params.limit,
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
