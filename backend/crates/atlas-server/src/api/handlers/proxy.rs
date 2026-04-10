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
use atlas_common::{AtlasError, ContractAbi, ProxyContract};

// EIP-1967 implementation slot: keccak256("eip1967.proxy.implementation") - 1
const EIP1967_IMPL_SLOT: &str =
    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";
// EIP-1822 (UUPS) implementation slot: keccak256("PROXIABLE")
const EIP1822_IMPL_SLOT: &str =
    "0xc5f16f0fcc639fa48a6947836d9850f504798523bf8c9a3a87d5876cf622bcf7";

/// Try to read a storage slot via eth_getStorageAt and return a non-zero address if found.
async fn read_address_slot(
    rpc_url: &str,
    address: &str,
    slot: &str,
) -> Result<Option<String>, AtlasError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getStorageAt",
        "params": [address, slot, "latest"],
        "id": 1
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| AtlasError::Internal(e.to_string()))?;

    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AtlasError::Rpc(format!("eth_getStorageAt failed: {e}")))?
        .json()
        .await
        .map_err(|e| AtlasError::Rpc(format!("failed to parse eth_getStorageAt response: {e}")))?;

    let raw = resp.get("result").and_then(|r| r.as_str()).unwrap_or("0x");

    // Result is 32 bytes; address occupies the last 20 bytes (40 hex chars).
    let hex = raw.trim_start_matches("0x");
    if hex.len() < 40 || hex.chars().all(|c| c == '0') {
        return Ok(None);
    }
    let addr = format!("0x{}", &hex[hex.len() - 40..]).to_lowercase();
    if addr == "0x0000000000000000000000000000000000000000" {
        return Ok(None);
    }
    Ok(Some(addr))
}

/// Detect a proxy pattern for `address` via RPC and, if found, persist it in `proxy_contracts`.
/// Returns the cached or newly detected `ProxyContract`, or `None` if not a proxy.
async fn resolve_proxy(
    state: &AppState,
    address: &str,
) -> Result<Option<ProxyContract>, AtlasError> {
    // 1. Check DB cache first.
    let cached: Option<ProxyContract> = sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address,
                detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts WHERE proxy_address = $1",
    )
    .bind(address)
    .fetch_optional(&state.pool)
    .await?;

    if cached.is_some() {
        return Ok(cached);
    }

    // 2. Not in DB — try RPC detection.
    let detected = if let Some(impl_addr) =
        read_address_slot(&state.rpc_url, address, EIP1967_IMPL_SLOT).await?
    {
        Some((impl_addr, "eip1967"))
    } else {
        read_address_slot(&state.rpc_url, address, EIP1822_IMPL_SLOT)
            .await?
            .map(|impl_addr| (impl_addr, "eip1822"))
    };

    let Some((impl_addr, proxy_type)) = detected else {
        return Ok(None);
    };

    // 3. Persist so future requests hit the DB cache.
    sqlx::query(
        "INSERT INTO proxy_contracts
            (proxy_address, implementation_address, proxy_type, detected_at_block, last_checked_block)
         VALUES ($1, $2, $3, 0, 0)
         ON CONFLICT (proxy_address) DO NOTHING",
    )
    .bind(address)
    .bind(&impl_addr)
    .bind(proxy_type)
    .execute(&state.pool)
    .await?;

    // 4. Re-fetch so the returned struct has the real DB timestamps.
    let proxy: Option<ProxyContract> = sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address,
                detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts WHERE proxy_address = $1",
    )
    .bind(address)
    .fetch_optional(&state.pool)
    .await?;

    Ok(proxy)
}

/// GET /api/contracts/:address/proxy - Get proxy information for a contract
pub async fn get_proxy_info(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<ProxyInfoResponse>> {
    let address = normalize_address(&address);

    let proxy = resolve_proxy(&state, &address).await?;

    // Check if this address is an implementation for any proxies
    let proxies_using_this: Vec<ProxyContract> = sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address, detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts
         WHERE implementation_address = $1",
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
             WHERE address = $1",
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

    // Resolve proxy (DB cache → RPC detection)
    let proxy = resolve_proxy(&state, &address).await?;

    // Get proxy ABI
    let proxy_abi: Option<ContractAbi> = sqlx::query_as(
        "SELECT address, abi, source_code, compiler_version, optimization_used, runs, verified_at
         FROM contract_abis
         WHERE address = $1",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    if let Some(proxy_info) = proxy {
        // Get implementation ABI
        let impl_abi: Option<ContractAbi> = sqlx::query_as(
            "SELECT address, abi, source_code, compiler_version, optimization_used, runs, verified_at
             FROM contract_abis
             WHERE address = $1",
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
