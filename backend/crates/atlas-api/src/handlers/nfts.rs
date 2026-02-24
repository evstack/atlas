use alloy::primitives::{Address, U256};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::error::ApiResult;
use crate::AppState;
use atlas_common::{AtlasError, NftContract, NftToken, NftTransfer, PaginatedResponse, Pagination};

/// NFT metadata JSON structure (ERC-721 standard)
#[derive(Debug, Deserialize, serde::Serialize)]
struct NftMetadata {
    name: Option<String>,
    description: Option<String>,
    image: Option<String>,
    #[serde(default)]
    attributes: Vec<serde_json::Value>,
}

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
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Collection {} not found", address)))?;

    // Fetch name/symbol on-demand if not already fetched
    if collection.name.is_none() && collection.symbol.is_none() {
        if let Ok((name, symbol)) = fetch_collection_metadata(&state.rpc_url, &address).await {
            // Update the database
            sqlx::query(
                "UPDATE nft_contracts SET name = $1, symbol = $2 WHERE LOWER(address) = LOWER($3)",
            )
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
    let provider = ProviderBuilder::new().on_http(url);

    // name() selector = 0x06fdde03
    let name = {
        let tx = TransactionRequest::default()
            .to(contract)
            .input(alloy::primitives::Bytes::from(vec![0x06, 0xfd, 0xde, 0x03]).into());
        provider
            .call(&tx)
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
            .call(&tx)
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
        sqlx::query_as("SELECT COUNT(*) FROM nft_tokens WHERE LOWER(contract_address) = LOWER($1)")
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

    let mut token: NftToken = sqlx::query_as(
        "SELECT contract_address, token_id, owner, token_uri, metadata_fetched, metadata, image_url, name, last_transfer_block
         FROM nft_tokens
         WHERE LOWER(contract_address) = LOWER($1) AND token_id = $2::numeric"
    )
    .bind(&address)
    .bind(&token_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AtlasError::NotFound(format!("Token {}:{} not found", address, token_id)))?;

    // Fetch metadata on-demand if not already fetched
    if !token.metadata_fetched {
        if let Ok(updated_token) = fetch_and_store_metadata(&state, &address, &token_id).await {
            token = updated_token;
        }
    }

    Ok(Json(token))
}

/// Fetch NFT metadata on-demand and store it in the database
async fn fetch_and_store_metadata(
    state: &AppState,
    contract_address: &str,
    token_id: &str,
) -> Result<NftToken, AtlasError> {
    // Parse contract address and token ID
    let contract_addr: Address = contract_address
        .parse()
        .map_err(|_| AtlasError::InvalidInput("Invalid contract address".to_string()))?;
    let token_id_u256 = U256::from_str_radix(token_id, 10)
        .map_err(|_| AtlasError::InvalidInput("Invalid token ID".to_string()))?;

    // Call tokenURI(uint256) on the contract
    let token_uri = fetch_token_uri(&state.rpc_url, contract_addr, token_id_u256).await;

    // If we got a token URI, fetch the metadata
    let (metadata_json, image_url, name) = if let Some(ref uri) = token_uri {
        match fetch_metadata_from_uri(uri).await {
            Ok(metadata) => {
                let image = metadata.image.as_ref().map(|img| resolve_ipfs_url(img));
                let name = metadata.name.clone();
                (
                    Some(serde_json::to_value(&metadata).unwrap_or_default()),
                    image,
                    name,
                )
            }
            Err(_) => (None, None, None),
        }
    } else {
        (None, None, None)
    };

    // Update the database
    sqlx::query(
        "UPDATE nft_tokens SET
            token_uri = $1,
            metadata_fetched = true,
            metadata = $2,
            image_url = $3,
            name = $4
         WHERE LOWER(contract_address) = LOWER($5) AND token_id = $6::numeric",
    )
    .bind(&token_uri)
    .bind(&metadata_json)
    .bind(&image_url)
    .bind(&name)
    .bind(contract_address)
    .bind(token_id)
    .execute(&state.pool)
    .await?;

    // Fetch and return the updated token
    let token: NftToken = sqlx::query_as(
        "SELECT contract_address, token_id, owner, token_uri, metadata_fetched, metadata, image_url, name, last_transfer_block
         FROM nft_tokens
         WHERE LOWER(contract_address) = LOWER($1) AND token_id = $2::numeric"
    )
    .bind(contract_address)
    .bind(token_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(token)
}

/// Call tokenURI(uint256) on an NFT contract
async fn fetch_token_uri(rpc_url: &str, contract: Address, token_id: U256) -> Option<String> {
    use alloy::providers::{Provider, ProviderBuilder};
    use alloy::rpc::types::TransactionRequest;

    let url: reqwest::Url = rpc_url.parse().ok()?;
    let provider = ProviderBuilder::new().on_http(url);

    // tokenURI(uint256) selector = 0xc87b56dd
    let mut calldata = vec![0xc8, 0x7b, 0x56, 0xdd];
    calldata.extend_from_slice(&token_id.to_be_bytes::<32>());

    let tx = TransactionRequest::default()
        .to(contract)
        .input(alloy::primitives::Bytes::from(calldata).into());

    let result = provider.call(&tx).await.ok()?;

    // Decode string from ABI encoding
    decode_abi_string(&result)
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

/// Fetch metadata JSON from a URI (handles IPFS)
async fn fetch_metadata_from_uri(uri: &str) -> Result<NftMetadata, AtlasError> {
    let url = resolve_ipfs_url(uri);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AtlasError::MetadataFetch(e.to_string()))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AtlasError::MetadataFetch(format!("Failed to fetch metadata: {}", e)))?;

    let metadata: NftMetadata = response
        .json()
        .await
        .map_err(|e| AtlasError::MetadataFetch(format!("Failed to parse metadata: {}", e)))?;

    Ok(metadata)
}

/// Convert IPFS URLs to HTTP gateway URLs
fn resolve_ipfs_url(uri: &str) -> String {
    if uri.starts_with("ipfs://") {
        // Convert ipfs://QmXxx... to https://ipfs.io/ipfs/QmXxx...
        format!("https://ipfs.io/ipfs/{}", &uri[7..])
    } else if uri.starts_with("ar://") {
        // Arweave URLs
        format!("https://arweave.net/{}", &uri[5..])
    } else {
        uri.to_string()
    }
}

/// GET /api/nfts/collections/{address}/transfers - Get all transfers for a collection
pub async fn get_collection_transfers(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<PaginatedResponse<NftTransfer>>> {
    let address = normalize_address(&address);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM nft_transfers WHERE LOWER(contract_address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_one(&state.pool)
    .await?;

    let transfers: Vec<NftTransfer> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, contract_address, token_id, from_address, to_address, block_number, timestamp
         FROM nft_transfers
         WHERE LOWER(contract_address) = LOWER($1)
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
