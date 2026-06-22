//! Proxy contract detection and API
//!
//! Detects and stores relationships between proxy contracts and their implementations.
//! Detection is done lazily on first request via `eth_getStorageAt` against known proxy slots,
//! and cached in `proxy_contracts` for subsequent requests.

use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use crate::api::error::ApiResult;
use crate::api::AppState;
use crate::contract_abi::{
    load_combined_abi, load_contract_abi, merge_abis, proxies_using_implementation, resolve_proxy,
};
use atlas_common::{ContractAbi, ProxyContract};

/// GET /api/contracts/:address/proxy - Get proxy information for a contract
pub async fn get_proxy_info(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<ProxyInfoResponse>> {
    let address = normalize_address(&address);

    let proxy = resolve_proxy(&state.pool, &state.rpc_url, &address).await?;

    // Check if this address is an implementation for any proxies
    let proxies_using_this = proxies_using_implementation(&state.pool, &address).await?;

    if proxy.is_none() && proxies_using_this.is_empty() {
        return Ok(Json(ProxyInfoResponse {
            is_proxy: false,
            is_implementation: false,
            proxy: None,
            implementation_abi: None,
            proxies_using_this: vec![],
        }));
    }

    // Get implementation ABI if this is a proxy
    let implementation_abi = if let Some(ref p) = proxy {
        load_contract_abi(&state.pool, &p.implementation_address).await?
    } else {
        None
    };

    Ok(Json(ProxyInfoResponse {
        is_proxy: proxy.is_some(),
        is_implementation: !proxies_using_this.is_empty(),
        proxy,
        implementation_abi,
        proxies_using_this,
    }))
}

/// Proxy information response
#[derive(Debug, serde::Serialize)]
pub struct ProxyInfoResponse {
    pub is_proxy: bool,
    pub is_implementation: bool,
    pub proxy: Option<ProxyContract>,
    pub implementation_abi: Option<ContractAbi>,
    pub proxies_using_this: Vec<ProxyContract>,
}

/// GET /api/contracts/:address/combined-abi - Get combined ABI (proxy + implementation)
pub async fn get_combined_abi(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<CombinedAbiResponse>> {
    let address = normalize_address(&address);

    // Resolve proxy (DB cache → RPC detection)
    let proxy = resolve_proxy(&state.pool, &state.rpc_url, &address).await?;

    // Get proxy ABI
    let proxy_abi = load_contract_abi(&state.pool, &address).await?;

    if let Some(proxy_info) = proxy {
        // Get implementation ABI
        let impl_abi = load_contract_abi(&state.pool, &proxy_info.implementation_address).await?;

        // Merge ABIs
        let combined = merge_abis(
            proxy_abi.as_ref().map(|a| &a.abi),
            impl_abi.as_ref().map(|a| &a.abi),
        );

        Ok(Json(CombinedAbiResponse {
            is_proxy: true,
            proxy_address: address,
            implementation_address: Some(proxy_info.implementation_address),
            proxy_type: Some(proxy_info.proxy_type),
            combined_abi: combined,
            proxy_abi: proxy_abi.map(|a| a.abi),
            implementation_abi: impl_abi.map(|a| a.abi),
        }))
    } else {
        // Not a proxy, just return the contract's ABI
        let combined = load_combined_abi(&state.pool, &state.rpc_url, &address)
            .await?
            .map(|resolved| resolved.abi);
        Ok(Json(CombinedAbiResponse {
            is_proxy: false,
            proxy_address: address,
            implementation_address: None,
            proxy_type: None,
            combined_abi: combined,
            proxy_abi: None,
            implementation_abi: proxy_abi.map(|a| a.abi),
        }))
    }
}

/// Combined ABI response
#[derive(Debug, serde::Serialize)]
pub struct CombinedAbiResponse {
    pub is_proxy: bool,
    pub proxy_address: String,
    pub implementation_address: Option<String>,
    pub proxy_type: Option<String>,
    pub combined_abi: Option<serde_json::Value>,
    pub proxy_abi: Option<serde_json::Value>,
    pub implementation_abi: Option<serde_json::Value>,
}

/// GET /api/proxies - List all known proxy contracts
pub async fn list_proxies(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(pagination): axum::extract::Query<atlas_common::Pagination>,
) -> ApiResult<Json<atlas_common::PaginatedResponse<ProxyContract>>> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM proxy_contracts")
        .fetch_one(&state.pool)
        .await?;

    let proxies: Vec<ProxyContract> = sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address, detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts
         ORDER BY detected_at_block DESC
         LIMIT $1 OFFSET $2",
    )
    .bind(pagination.limit())
    .bind(pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(atlas_common::PaginatedResponse::new(
        proxies,
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
