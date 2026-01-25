use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use atlas_common::{AtlasError, Transaction};
use crate::AppState;
use crate::error::ApiResult;

pub async fn get_transaction(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> ApiResult<Json<Transaction>> {
    // Normalize hash format
    let hash = if hash.starts_with("0x") { hash } else { format!("0x{}", hash) };

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
