//! Proxy contract detection and API
//!
//! Detects and stores relationships between proxy contracts and their implementations.

use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use crate::api::error::ApiResult;
use crate::api::AppState;
use atlas_common::{ContractAbi, ProxyContract};

/// GET /api/contracts/:address/proxy - Get proxy information for a contract
pub async fn get_proxy_info(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<ProxyInfoResponse>> {
    let address = normalize_address(&address);

    // Check if this address is a proxy
    let proxy: Option<ProxyContract> = sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address, detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts
         WHERE LOWER(proxy_address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    // Check if this address is an implementation for any proxies
    let proxies_using_this: Vec<ProxyContract> = sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address, detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts
         WHERE LOWER(implementation_address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?
    .map(|p| vec![p])
    .unwrap_or_default();

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
        sqlx::query_as::<_, ContractAbi>(
            "SELECT address, abi, source_code, compiler_version, optimization_used, runs, verified_at
             FROM contract_abis
             WHERE LOWER(address) = LOWER($1)",
        )
        .bind(&p.implementation_address)
        .fetch_optional(&state.pool)
        .await?
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

    // Check if this is a proxy
    let proxy: Option<ProxyContract> = sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address, detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts
         WHERE LOWER(proxy_address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    // Get proxy ABI
    let proxy_abi: Option<ContractAbi> = sqlx::query_as(
        "SELECT address, abi, source_code, compiler_version, optimization_used, runs, verified_at
         FROM contract_abis
         WHERE LOWER(address) = LOWER($1)",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    if let Some(proxy_info) = proxy {
        // Get implementation ABI
        let impl_abi: Option<ContractAbi> = sqlx::query_as(
            "SELECT address, abi, source_code, compiler_version, optimization_used, runs, verified_at
             FROM contract_abis
             WHERE LOWER(address) = LOWER($1)",
        )
        .bind(&proxy_info.implementation_address)
        .fetch_optional(&state.pool)
        .await?;

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
        Ok(Json(CombinedAbiResponse {
            is_proxy: false,
            proxy_address: address,
            implementation_address: None,
            proxy_type: None,
            combined_abi: proxy_abi.as_ref().map(|a| a.abi.clone()),
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

/// Merge proxy and implementation ABIs
fn merge_abis(
    proxy_abi: Option<&serde_json::Value>,
    impl_abi: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    match (proxy_abi, impl_abi) {
        (Some(proxy), Some(implementation)) => {
            // Both ABIs exist - merge them
            let mut merged = Vec::new();

            // Add all implementation functions/events (these are the main ones)
            if let Some(impl_arr) = implementation.as_array() {
                merged.extend(impl_arr.clone());
            }

            // Add proxy-specific functions that aren't in implementation
            // (like upgradeTo, admin, etc.)
            if let Some(proxy_arr) = proxy.as_array() {
                let impl_names: std::collections::HashSet<String> = merged
                    .iter()
                    .filter_map(|item| item.get("name").and_then(|n| n.as_str()))
                    .map(String::from)
                    .collect();

                for item in proxy_arr {
                    if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                        if !impl_names.contains(name) {
                            merged.push(item.clone());
                        }
                    } else {
                        // Include items without names (like fallback, receive)
                        merged.push(item.clone());
                    }
                }
            }

            Some(serde_json::Value::Array(merged))
        }
        (Some(abi), None) | (None, Some(abi)) => Some(abi.clone()),
        (None, None) => None,
    }
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
