use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use atlas_common::{Block, Transaction, Address, NftToken};
use crate::AppState;
use crate::error::ApiResult;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum SearchResult {
    #[serde(rename = "block")]
    Block(Block),
    #[serde(rename = "transaction")]
    Transaction(Transaction),
    #[serde(rename = "address")]
    Address(Address),
    #[serde(rename = "nft")]
    Nft(NftToken),
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub query: String,
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> ApiResult<Json<SearchResponse>> {
    let query = params.q.trim();
    let mut results = Vec::new();

    // Detect query type
    if query.is_empty() {
        return Ok(Json(SearchResponse { results, query: query.to_string() }));
    }

    // Check if it's a hex string (address or hash)
    if query.starts_with("0x") || query.chars().all(|c| c.is_ascii_hexdigit()) {
        let hex_query = if query.starts_with("0x") { query.to_string() } else { format!("0x{}", query) };

        // 42 chars = address (0x + 40 hex)
        if hex_query.len() == 42 {
            if let Some(addr) = search_address(&state, &hex_query).await? {
                results.push(SearchResult::Address(addr));
            }
        }

        // 66 chars = tx hash or block hash (0x + 64 hex)
        if hex_query.len() == 66 {
            // Try transaction first
            if let Some(tx) = search_transaction(&state, &hex_query).await? {
                results.push(SearchResult::Transaction(tx));
            }
            // Try block by hash
            if let Some(block) = search_block_by_hash(&state, &hex_query).await? {
                results.push(SearchResult::Block(block));
            }
        }
    }

    // Check if it's a block number
    if let Ok(block_num) = query.parse::<i64>() {
        if let Some(block) = search_block_by_number(&state, block_num).await? {
            results.push(SearchResult::Block(block));
        }
    }

    // Search NFT names (full-text search)
    if results.is_empty() {
        let nfts = search_nft_by_name(&state, query).await?;
        for nft in nfts {
            results.push(SearchResult::Nft(nft));
        }
    }

    Ok(Json(SearchResponse { results, query: query.to_string() }))
}

async fn search_address(state: &AppState, address: &str) -> Result<Option<Address>, atlas_common::AtlasError> {
    sqlx::query_as(
        "SELECT address, is_contract, first_seen_block, tx_count
         FROM addresses
         WHERE LOWER(address) = LOWER($1)"
    )
    .bind(address)
    .fetch_optional(&state.pool)
    .await
    .map_err(Into::into)
}

async fn search_transaction(state: &AppState, hash: &str) -> Result<Option<Transaction>, atlas_common::AtlasError> {
    sqlx::query_as(
        "SELECT hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, contract_created, timestamp
         FROM transactions
         WHERE LOWER(hash) = LOWER($1)"
    )
    .bind(hash)
    .fetch_optional(&state.pool)
    .await
    .map_err(Into::into)
}

async fn search_block_by_hash(state: &AppState, hash: &str) -> Result<Option<Block>, atlas_common::AtlasError> {
    sqlx::query_as(
        "SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at
         FROM blocks
         WHERE LOWER(hash) = LOWER($1)"
    )
    .bind(hash)
    .fetch_optional(&state.pool)
    .await
    .map_err(Into::into)
}

async fn search_block_by_number(state: &AppState, number: i64) -> Result<Option<Block>, atlas_common::AtlasError> {
    sqlx::query_as(
        "SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at
         FROM blocks
         WHERE number = $1"
    )
    .bind(number)
    .fetch_optional(&state.pool)
    .await
    .map_err(Into::into)
}

async fn search_nft_by_name(state: &AppState, query: &str) -> Result<Vec<NftToken>, atlas_common::AtlasError> {
    // Use ILIKE for simple search, could be replaced with full-text search
    let pattern = format!("%{}%", query);
    sqlx::query_as(
        "SELECT contract_address, token_id, owner, token_uri, metadata_fetched, metadata, image_url, name, last_transfer_block
         FROM nft_tokens
         WHERE name ILIKE $1
         LIMIT 10"
    )
    .bind(&pattern)
    .fetch_all(&state.pool)
    .await
    .map_err(Into::into)
}
