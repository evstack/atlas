use alloy::primitives::{Address, U256};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

use crate::api::error::ApiResult;
use crate::api::AppState;
use atlas_common::{AtlasError, NftContract, NftToken, NftTransfer, PaginatedResponse, Pagination};

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
         LIMIT $1 OFFSET $2",
    )
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        collections,
        pagination.page,
        pagination.limit,
        total.0,
    )))
}

pub async fn get_collection(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<NftContract>> {
    let address = normalize_address(&address);

    let mut collection: NftContract = sqlx::query_as(
        "SELECT address, name, symbol, total_supply, first_seen_block
         FROM nft_contracts
         WHERE address = $1",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Collection {} not found", address)))?;

    // Fetch name/symbol on-demand if not already fetched
    if collection.name.is_none() && collection.symbol.is_none() {
        if let Ok((name, symbol)) = fetch_collection_metadata(&state.rpc_url, &address).await {
            // Update the database
            sqlx::query("UPDATE nft_contracts SET name = $1, symbol = $2 WHERE address = $3")
                .bind(&name)
                .bind(&symbol)
                .bind(&address)
                .execute(&state.pool)
                .await?;

            collection.name = name;
            collection.symbol = symbol;
        }
    }

    Ok(Json(collection))
}

/// Fetch NFT collection name and symbol from contract
async fn fetch_collection_metadata(
    rpc_url: &str,
    contract_address: &str,
) -> Result<(Option<String>, Option<String>), AtlasError> {
    use alloy::providers::{Provider, ProviderBuilder};
    use alloy::rpc::types::TransactionRequest;

    let contract: Address = contract_address
        .parse()
        .map_err(|_| AtlasError::InvalidInput("Invalid contract address".to_string()))?;

    let url: reqwest::Url = rpc_url
        .parse()
        .map_err(|_| AtlasError::InvalidInput("Invalid RPC URL".to_string()))?;
    let provider = ProviderBuilder::new().connect_http(url);

    // name() selector = 0x06fdde03
    let name = {
        let tx = TransactionRequest::default()
            .to(contract)
            .input(alloy::primitives::Bytes::from(vec![0x06, 0xfd, 0xde, 0x03]).into());
        provider
            .call(tx)
            .await
            .ok()
            .and_then(|r| decode_abi_string(&r))
    };

    // symbol() selector = 0x95d89b41
    let symbol = {
        let tx = TransactionRequest::default()
            .to(contract)
            .input(alloy::primitives::Bytes::from(vec![0x95, 0xd8, 0x9b, 0x41]).into());
        provider
            .call(tx)
            .await
            .ok()
            .and_then(|r| decode_abi_string(&r))
    };

    Ok((name, symbol))
}

pub async fn list_collection_tokens(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<NftToken>>> {
    let address = normalize_address(&address);

    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM nft_tokens WHERE contract_address = $1")
            .bind(&address)
            .fetch_one(&state.pool)
            .await?;

    let tokens: Vec<NftToken> = sqlx::query_as(
        "SELECT contract_address, token_id, owner, token_uri, metadata_status, metadata_retry_count,
                next_retry_at, last_metadata_error, last_metadata_attempted_at, metadata_updated_at,
                metadata, image_url, name, last_transfer_block
         FROM nft_tokens
         WHERE contract_address = $1
         ORDER BY token_id ASC
         LIMIT $2 OFFSET $3"
    )
    .bind(&address)
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

pub async fn get_token(
    State(state): State<Arc<AppState>>,
    Path((address, token_id)): Path<(String, String)>,
) -> ApiResult<Json<NftToken>> {
    let address = normalize_address(&address);

    let token: NftToken = sqlx::query_as(
        "SELECT contract_address, token_id, owner, token_uri, metadata_status, metadata_retry_count,
                next_retry_at, last_metadata_error, last_metadata_attempted_at, metadata_updated_at,
                metadata, image_url, name, last_transfer_block
         FROM nft_tokens
         WHERE contract_address = $1 AND token_id = $2::numeric"
    )
    .bind(&address)
    .bind(&token_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Token {}:{} not found", address, token_id)))?;

    Ok(Json(token))
}

/// Decode an ABI-encoded string
fn decode_abi_string(data: &[u8]) -> Option<String> {
    if data.len() < 64 {
        return None;
    }

    // Offset is at bytes 0-32
    let offset = U256::from_be_slice(&data[0..32]).to::<usize>();
    if offset + 32 > data.len() {
        return None;
    }

    // Length is at offset position
    let length = U256::from_be_slice(&data[offset..offset + 32]).to::<usize>();
    if offset + 32 + length > data.len() {
        return None;
    }

    // String data follows
    let string_data = &data[offset + 32..offset + 32 + length];
    String::from_utf8(string_data.to_vec()).ok()
}

/// GET /api/nfts/collections/{address}/transfers - Get all transfers for a collection
pub async fn get_collection_transfers(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<NftTransfer>>> {
    let address = normalize_address(&address);

    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM nft_transfers WHERE contract_address = $1")
            .bind(&address)
            .fetch_one(&state.pool)
            .await?;

    let transfers: Vec<NftTransfer> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, contract_address, token_id, from_address, to_address, block_number, timestamp
         FROM nft_transfers
         WHERE contract_address = $1
         ORDER BY block_number DESC, log_index DESC
         LIMIT $2 OFFSET $3"
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

/// GET /api/nfts/collections/{address}/tokens/{token_id}/transfers - Get transfers for a specific token
pub async fn get_token_transfers(
    State(state): State<Arc<AppState>>,
    Path((address, token_id)): Path<(String, String)>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<NftTransfer>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM nft_transfers WHERE contract_address = $1 AND token_id = $2::numeric",
    )
    .bind(&address)
    .bind(&token_id)
    .fetch_one(&state.pool)
    .await?;

    let transfers: Vec<NftTransfer> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, contract_address, token_id, from_address, to_address, block_number, timestamp
         FROM nft_transfers
         WHERE contract_address = $1 AND token_id = $2::numeric
         ORDER BY block_number DESC, log_index DESC
         LIMIT $3 OFFSET $4"
    )
    .bind(&address)
    .bind(&token_id)
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

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}
