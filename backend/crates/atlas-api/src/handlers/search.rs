use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::ApiResult;
use crate::AppState;
use atlas_common::{Address, Block, Erc20Contract, NftContract, Transaction};

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
    #[serde(rename = "nft_collection")]
    NftCollection(NftContract),
    #[serde(rename = "erc20_token")]
    Erc20Token(Erc20Contract),
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

    if query.is_empty() {
        return Ok(Json(SearchResponse {
            results,
            query: query.to_string(),
        }));
    }

    // Detect query type and run appropriate searches in parallel
    let is_hex = query.starts_with("0x") || query.chars().all(|c| c.is_ascii_hexdigit());
    let block_num = query.parse::<i64>().ok();

    if is_hex {
        let hex_query = if query.starts_with("0x") {
            query.to_lowercase()
        } else {
            format!("0x{}", query.to_lowercase())
        };

        match hex_query.len() {
            // 42 chars = address (0x + 40 hex)
            42 => {
                if let Some(addr) = search_address(&state, &hex_query).await? {
                    results.push(SearchResult::Address(addr));
                }
            }
            // 66 chars = tx hash or block hash (0x + 64 hex)
            66 => {
                // Run tx and block search in parallel
                let (tx_result, block_result) = tokio::join!(
                    search_transaction(&state, &hex_query),
                    search_block_by_hash(&state, &hex_query)
                );

                if let Some(tx) = tx_result? {
                    results.push(SearchResult::Transaction(tx));
                }
                if let Some(block) = block_result? {
                    results.push(SearchResult::Block(block));
                }
            }
            _ => {}
        }
    } else if let Some(num) = block_num {
        // Block number search
        if let Some(block) = search_block_by_number(&state, num).await? {
            results.push(SearchResult::Block(block));
        }
    }

    // Text search for tokens/collections if no hex/number results and query is meaningful
    if results.is_empty() && query.len() >= 2 {
        // Run NFT and ERC-20 searches in parallel
        let (nft_results, erc20_results) = tokio::join!(
            search_nft_collections(&state, query),
            search_erc20_tokens(&state, query)
        );

        for nft in nft_results? {
            results.push(SearchResult::NftCollection(nft));
        }
        for token in erc20_results? {
            results.push(SearchResult::Erc20Token(token));
        }
    }

    Ok(Json(SearchResponse {
        results,
        query: query.to_string(),
    }))
}

async fn search_address(
    state: &AppState,
    address: &str,
) -> Result<Option<Address>, atlas_common::AtlasError> {
    // Address is already lowercased by caller
    sqlx::query_as(
        "SELECT address, is_contract, first_seen_block, tx_count
         FROM addresses
         WHERE address = $1",
    )
    .bind(address)
    .fetch_optional(&state.pool)
    .await
    .map_err(Into::into)
}

async fn search_transaction(
    state: &AppState,
    hash: &str,
) -> Result<Option<Transaction>, atlas_common::AtlasError> {
    // Use tx_hash_lookup table for O(1) lookup, then fetch full tx with partition key
    sqlx::query_as(
        "SELECT t.hash, t.block_number, t.block_index, t.from_address, t.to_address, t.value, t.gas_price, t.gas_used, t.input_data, t.status, t.contract_created, t.timestamp
         FROM tx_hash_lookup l
         JOIN transactions t ON t.hash = l.hash AND t.block_number = l.block_number
         WHERE l.hash = $1"
    )
    .bind(hash)
    .fetch_optional(&state.pool)
    .await
    .map_err(Into::into)
}

async fn search_block_by_hash(
    state: &AppState,
    hash: &str,
) -> Result<Option<Block>, atlas_common::AtlasError> {
    // Hash is already lowercased by caller
    sqlx::query_as(
        "SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at
         FROM blocks
         WHERE hash = $1"
    )
    .bind(hash)
    .fetch_optional(&state.pool)
    .await
    .map_err(Into::into)
}

async fn search_block_by_number(
    state: &AppState,
    number: i64,
) -> Result<Option<Block>, atlas_common::AtlasError> {
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

async fn search_nft_collections(
    state: &AppState,
    query: &str,
) -> Result<Vec<NftContract>, atlas_common::AtlasError> {
    let pattern = format!("%{}%", query);
    sqlx::query_as(
        "SELECT address, name, symbol, total_supply, first_seen_block
         FROM nft_contracts
         WHERE name ILIKE $1 OR symbol ILIKE $1
         ORDER BY total_supply DESC NULLS LAST
         LIMIT 10",
    )
    .bind(&pattern)
    .fetch_all(&state.pool)
    .await
    .map_err(Into::into)
}

async fn search_erc20_tokens(
    state: &AppState,
    query: &str,
) -> Result<Vec<Erc20Contract>, atlas_common::AtlasError> {
    let pattern = format!("%{}%", query);
    sqlx::query_as(
        "SELECT address, name, symbol, decimals, total_supply, first_seen_block
         FROM erc20_contracts
         WHERE name ILIKE $1 OR symbol ILIKE $1
         ORDER BY first_seen_block DESC
         LIMIT 10",
    )
    .bind(&pattern)
    .fetch_all(&state.pool)
    .await
    .map_err(Into::into)
}
