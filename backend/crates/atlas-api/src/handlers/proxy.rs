//! Proxy contract detection and API
//!
//! Detects and stores relationships between proxy contracts and their implementations.

use alloy::providers::Provider;
use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use atlas_common::{AtlasError, ContractAbi, ProxyContract};
use crate::AppState;
use crate::error::ApiResult;

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

/// POST /api/contracts/:address/detect-proxy - Trigger proxy detection for a contract
pub async fn detect_proxy(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<ProxyDetectionResult>> {
    let address = normalize_address(&address);

    // Get RPC provider
    use alloy::providers::{Provider, ProviderBuilder};

    let provider = ProviderBuilder::new()
        .on_http(state.rpc_url.parse().map_err(|e| AtlasError::Config(format!("Invalid RPC URL: {}", e)))?);

    let addr: alloy::primitives::Address = address.parse()
        .map_err(|_| AtlasError::InvalidInput("Invalid address".to_string()))?;

    // Check known proxy storage slots
    let result = detect_proxy_impl(&provider, addr).await?;

    if let Some((impl_addr, proxy_type, admin_addr)) = result {
        // Get current block
        let current_block = provider.get_block_number().await
            .map_err(|e| AtlasError::Rpc(e.to_string()))?;

        // Store in database
        sqlx::query(
            "INSERT INTO proxy_contracts (proxy_address, implementation_address, proxy_type, admin_address, detected_at_block, last_checked_block, updated_at)
             VALUES ($1, $2, $3, $4, $5, $5, NOW())
             ON CONFLICT (proxy_address) DO UPDATE SET
                implementation_address = $2,
                proxy_type = $3,
                admin_address = $4,
                last_checked_block = $5,
                updated_at = NOW()",
        )
        .bind(&address)
        .bind(format!("{:?}", impl_addr))
        .bind(proxy_type.as_str())
        .bind(admin_addr.map(|a| format!("{:?}", a)))
        .bind(current_block as i64)
        .execute(&state.pool)
        .await?;

        Ok(Json(ProxyDetectionResult {
            is_proxy: true,
            proxy_type: Some(proxy_type.to_string()),
            implementation_address: Some(format!("{:?}", impl_addr)),
            admin_address: admin_addr.map(|a| format!("{:?}", a)),
        }))
    } else {
        Ok(Json(ProxyDetectionResult {
            is_proxy: false,
            proxy_type: None,
            implementation_address: None,
            admin_address: None,
        }))
    }
}

/// Proxy detection result
#[derive(Debug, serde::Serialize)]
pub struct ProxyDetectionResult {
    pub is_proxy: bool,
    pub proxy_type: Option<String>,
    pub implementation_address: Option<String>,
    pub admin_address: Option<String>,
}

/// Known proxy storage slots
mod slots {
    use alloy::primitives::B256;

    /// EIP-1967 Implementation slot
    /// keccak256("eip1967.proxy.implementation") - 1
    pub const EIP1967_IMPL: B256 = B256::new([
        0x36, 0x08, 0x94, 0xa1, 0x3b, 0xa1, 0xa3, 0x21, 0x06, 0x67, 0xc8, 0x28, 0x49, 0x2d, 0xb9,
        0x8d, 0xca, 0x3e, 0x20, 0x76, 0xcc, 0x37, 0x35, 0xa9, 0x20, 0xa3, 0xca, 0x50, 0x5d, 0x38,
        0x2b, 0xbc,
    ]);

    /// EIP-1967 Admin slot
    /// keccak256("eip1967.proxy.admin") - 1
    pub const EIP1967_ADMIN: B256 = B256::new([
        0xb5, 0x31, 0x27, 0x68, 0x4a, 0x56, 0x8b, 0x31, 0x73, 0xae, 0x13, 0xb9, 0xf8, 0xa6, 0x01,
        0x6e, 0x24, 0x3e, 0x63, 0xb6, 0xe8, 0xee, 0x11, 0x78, 0xd6, 0xa7, 0x17, 0x85, 0x0b, 0x5d,
        0x61, 0x03,
    ]);

    /// EIP-1822 (UUPS) Implementation slot
    /// keccak256("PROXIABLE")
    pub const EIP1822_IMPL: B256 = B256::new([
        0xc5, 0xf1, 0x6f, 0x0f, 0xcc, 0x63, 0x9f, 0xa4, 0x8a, 0x69, 0x47, 0x83, 0x6d, 0x98, 0x50,
        0xf5, 0x04, 0x79, 0x85, 0x23, 0xbf, 0x8c, 0x9a, 0x3a, 0x87, 0xd5, 0x87, 0x6c, 0xf6, 0x22,
        0xbc, 0xf7,
    ]);
}

/// Detect proxy implementation from storage slots
async fn detect_proxy_impl(
    provider: &alloy::providers::RootProvider<alloy::transports::http::Http<alloy::transports::http::Client>, alloy::network::Ethereum>,
    address: alloy::primitives::Address,
) -> Result<Option<(alloy::primitives::Address, atlas_common::ProxyType, Option<alloy::primitives::Address>)>, AtlasError> {
    use alloy::primitives::Address;

    // Check EIP-1967 implementation slot
    let impl_slot = provider
        .get_storage_at(address, slots::EIP1967_IMPL.into())
        .await
        .map_err(|e| AtlasError::Rpc(e.to_string()))?;

    if !impl_slot.is_zero() {
        // Convert U256 to bytes and extract address from last 20 bytes
        let bytes = impl_slot.to_be_bytes::<32>();
        let impl_addr = Address::from_slice(&bytes[12..]);
        if !impl_addr.is_zero() {
            // Check for admin
            let admin_addr = if let Ok(admin_slot) = provider
                .get_storage_at(address, slots::EIP1967_ADMIN.into())
                .await
            {
                if !admin_slot.is_zero() {
                    let admin_bytes = admin_slot.to_be_bytes::<32>();
                    let addr = Address::from_slice(&admin_bytes[12..]);
                    if !addr.is_zero() { Some(addr) } else { None }
                } else {
                    None
                }
            } else {
                None
            };

            return Ok(Some((impl_addr, atlas_common::ProxyType::Eip1967, admin_addr)));
        }
    }

    // Check EIP-1822 (UUPS) slot
    let uups_slot = provider
        .get_storage_at(address, slots::EIP1822_IMPL.into())
        .await
        .map_err(|e| AtlasError::Rpc(e.to_string()))?;

    if !uups_slot.is_zero() {
        let bytes = uups_slot.to_be_bytes::<32>();
        let impl_addr = Address::from_slice(&bytes[12..]);
        if !impl_addr.is_zero() {
            return Ok(Some((impl_addr, atlas_common::ProxyType::Eip1822, None)));
        }
    }

    Ok(None)
}

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}
