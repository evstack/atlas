use atlas_common::{AtlasError, ContractAbi, ProxyContract};
use sqlx::PgPool;

pub const DIRECT_ABI_SOURCE: &str = "direct_abi";
pub const PROXY_COMBINED_ABI_SOURCE: &str = "proxy_combined_abi";

// EIP-1967 implementation slot: keccak256("eip1967.proxy.implementation") - 1
const EIP1967_IMPL_SLOT: &str =
    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";
// EIP-1822 (UUPS) implementation slot: keccak256("PROXIABLE")
const EIP1822_IMPL_SLOT: &str =
    "0xc5f16f0fcc639fa48a6947836d9850f504798523bf8c9a3a87d5876cf622bcf7";

#[derive(Debug, Clone)]
pub struct ResolvedContractAbi {
    pub abi: serde_json::Value,
    pub source: &'static str,
}

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
pub async fn resolve_proxy(
    pool: &PgPool,
    rpc_url: &str,
    address: &str,
) -> Result<Option<ProxyContract>, AtlasError> {
    // 1. Check DB cache first.
    let cached: Option<ProxyContract> = sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address,
                detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts WHERE proxy_address = $1",
    )
    .bind(address)
    .fetch_optional(pool)
    .await?;

    if let Some(mut cached_proxy) = cached {
        // Re-read the implementation slot to detect upgrades.
        let current_impl = match cached_proxy.proxy_type.as_str() {
            "eip1967" => read_address_slot(rpc_url, address, EIP1967_IMPL_SLOT).await?,
            "eip1822" => read_address_slot(rpc_url, address, EIP1822_IMPL_SLOT).await?,
            _ => None,
        };

        if let Some(current_addr) = current_impl {
            if current_addr != cached_proxy.implementation_address {
                sqlx::query(
                    "UPDATE proxy_contracts SET implementation_address = $1, updated_at = NOW()
                     WHERE proxy_address = $2",
                )
                .bind(&current_addr)
                .bind(address)
                .execute(pool)
                .await?;
                cached_proxy.implementation_address = current_addr;
            }
        }

        return Ok(Some(cached_proxy));
    }

    // 2. Not in DB — try RPC detection.
    let detected = if let Some(implementation_address) =
        read_address_slot(rpc_url, address, EIP1967_IMPL_SLOT).await?
    {
        Some((implementation_address, "eip1967"))
    } else {
        read_address_slot(rpc_url, address, EIP1822_IMPL_SLOT)
            .await?
            .map(|implementation_address| (implementation_address, "eip1822"))
    };

    let Some((implementation_address, proxy_type)) = detected else {
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
    .bind(&implementation_address)
    .bind(proxy_type)
    .execute(pool)
    .await?;

    // 4. Re-fetch so the returned struct has the real DB timestamps.
    let proxy: Option<ProxyContract> = sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address,
                detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts WHERE proxy_address = $1",
    )
    .bind(address)
    .fetch_optional(pool)
    .await?;

    Ok(proxy)
}

pub fn merge_abis(
    proxy_abi: Option<&serde_json::Value>,
    implementation_abi: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    match (proxy_abi, implementation_abi) {
        (Some(proxy), Some(implementation)) => {
            let mut merged = Vec::new();
            if let Some(implementation_items) = implementation.as_array() {
                merged.extend(implementation_items.clone());
            }
            if let Some(proxy_items) = proxy.as_array() {
                merged.extend(proxy_items.clone());
            }

            let merged_value = serde_json::Value::Array(merged);
            match serde_json::from_value::<alloy::json_abi::JsonAbi>(merged_value.clone()) {
                Ok(mut abi) => {
                    abi.dedup();
                    serde_json::to_value(abi).ok().or(Some(merged_value))
                }
                Err(_) => Some(merged_value),
            }
        }
        (Some(abi), None) | (None, Some(abi)) => Some(abi.clone()),
        (None, None) => None,
    }
}

pub async fn load_contract_abi(
    pool: &PgPool,
    address: &str,
) -> Result<Option<ContractAbi>, AtlasError> {
    sqlx::query_as(
        "SELECT address, abi, source_code, compiler_version, optimization_used, runs, verified_at
         FROM contract_abis
         WHERE address = $1",
    )
    .bind(address)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn load_combined_abi(
    pool: &PgPool,
    rpc_url: &str,
    address: &str,
) -> Result<Option<ResolvedContractAbi>, AtlasError> {
    let proxy = resolve_proxy(pool, rpc_url, address).await?;
    let direct_abi = load_contract_abi(pool, address).await?;

    if let Some(proxy_info) = proxy {
        let implementation_abi =
            load_contract_abi(pool, &proxy_info.implementation_address).await?;
        let combined = merge_abis(
            direct_abi.as_ref().map(|abi| &abi.abi),
            implementation_abi.as_ref().map(|abi| &abi.abi),
        );

        Ok(combined.map(|abi| ResolvedContractAbi {
            abi,
            source: PROXY_COMBINED_ABI_SOURCE,
        }))
    } else {
        Ok(direct_abi.map(|abi| ResolvedContractAbi {
            abi: abi.abi,
            source: DIRECT_ABI_SOURCE,
        }))
    }
}

pub async fn proxies_using_implementation(
    pool: &PgPool,
    implementation_address: &str,
) -> Result<Vec<ProxyContract>, AtlasError> {
    sqlx::query_as(
        "SELECT proxy_address, implementation_address, proxy_type, admin_address,
                detected_at_block, last_checked_block, updated_at
         FROM proxy_contracts
         WHERE implementation_address = $1",
    )
    .bind(implementation_address)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}
