use alloy::{
    network::Ethereum,
    primitives::{Address, U256},
    providers::{ProviderBuilder, RootProvider},
    sol,
    transports::http::{Client, Http},
};
use anyhow::Result;
use bigdecimal::BigDecimal;
use sqlx::PgPool;
use std::{str::FromStr, sync::Arc, time::Duration};

use crate::config::Config;

// ERC-721 interface
sol! {
    #[sol(rpc)]
    interface IERC721Metadata {
        function tokenURI(uint256 tokenId) external view returns (string memory);
        function name() external view returns (string memory);
        function symbol() external view returns (string memory);
        function totalSupply() external view returns (uint256);
    }
}

// ERC-20 interface
sol! {
    #[sol(rpc)]
    interface IERC20Metadata {
        function name() external view returns (string memory);
        function symbol() external view returns (string memory);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
    }
}

type HttpProvider = RootProvider<Http<Client>, Ethereum>;

pub struct MetadataFetcher {
    pool: PgPool,
    config: Config,
    client: reqwest::Client,
    provider: Arc<HttpProvider>,
}

impl MetadataFetcher {
    pub fn new(pool: PgPool, config: Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let provider = Arc::new(ProviderBuilder::new().on_http(config.rpc_url.parse()?));

        Ok(Self { pool, config, client, provider })
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!("Starting metadata fetcher with {} workers", self.config.metadata_fetch_workers);

        loop {
            let mut did_work = false;

            // Phase 1: Fetch NFT contract metadata
            did_work |= self.fetch_nft_contract_metadata().await?;

            // Phase 2: Fetch ERC-20 contract metadata
            did_work |= self.fetch_erc20_contract_metadata().await?;

            // Phase 3: Fetch individual NFT token metadata
            did_work |= self.fetch_nft_token_metadata().await?;

            if !did_work {
                // No work, sleep
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }

    /// Fetch metadata for NFT contracts (name, symbol, totalSupply)
    async fn fetch_nft_contract_metadata(&self) -> Result<bool> {
        let contracts: Vec<(String,)> = sqlx::query_as(
            "SELECT address FROM nft_contracts WHERE metadata_fetched = false LIMIT $1"
        )
        .bind(self.config.metadata_fetch_workers as i32 * 5)
        .fetch_all(&self.pool)
        .await?;

        if contracts.is_empty() {
            return Ok(false);
        }

        tracing::debug!("Fetching metadata for {} NFT contracts", contracts.len());

        let mut handles = Vec::new();
        for (address,) in contracts {
            let pool = self.pool.clone();
            let provider = self.provider.clone();

            handles.push(tokio::spawn(async move {
                if let Err(e) = fetch_nft_contract_metadata(&pool, &provider, &address).await {
                    tracing::debug!("Failed to fetch NFT contract metadata for {}: {}", address, e);
                    // Mark as fetched to avoid infinite retries
                    let _ = sqlx::query("UPDATE nft_contracts SET metadata_fetched = true WHERE address = $1")
                        .bind(&address)
                        .execute(&pool)
                        .await;
                }
            }));

            if handles.len() >= self.config.metadata_fetch_workers as usize {
                for handle in handles.drain(..) {
                    let _ = handle.await;
                }
            }
        }

        for handle in handles {
            let _ = handle.await;
        }

        Ok(true)
    }

    /// Fetch metadata for ERC-20 contracts (name, symbol, decimals, totalSupply)
    async fn fetch_erc20_contract_metadata(&self) -> Result<bool> {
        let contracts: Vec<(String,)> = sqlx::query_as(
            "SELECT address FROM erc20_contracts WHERE metadata_fetched = false LIMIT $1"
        )
        .bind(self.config.metadata_fetch_workers as i32 * 5)
        .fetch_all(&self.pool)
        .await?;

        if contracts.is_empty() {
            return Ok(false);
        }

        tracing::debug!("Fetching metadata for {} ERC-20 contracts", contracts.len());

        let mut handles = Vec::new();
        for (address,) in contracts {
            let pool = self.pool.clone();
            let provider = self.provider.clone();

            handles.push(tokio::spawn(async move {
                if let Err(e) = fetch_erc20_contract_metadata(&pool, &provider, &address).await {
                    tracing::debug!("Failed to fetch ERC-20 contract metadata for {}: {}", address, e);
                    // Mark as fetched to avoid infinite retries
                    let _ = sqlx::query("UPDATE erc20_contracts SET metadata_fetched = true WHERE address = $1")
                        .bind(&address)
                        .execute(&pool)
                        .await;
                }
            }));

            if handles.len() >= self.config.metadata_fetch_workers as usize {
                for handle in handles.drain(..) {
                    let _ = handle.await;
                }
            }
        }

        for handle in handles {
            let _ = handle.await;
        }

        Ok(true)
    }

    /// Fetch metadata for individual NFT tokens
    async fn fetch_nft_token_metadata(&self) -> Result<bool> {
        let tokens: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT contract_address, token_id::text, token_uri
             FROM nft_tokens
             WHERE metadata_fetched = false
             LIMIT $1"
        )
        .bind(self.config.metadata_fetch_workers as i32 * 10)
        .fetch_all(&self.pool)
        .await?;

        if tokens.is_empty() {
            return Ok(false);
        }

        tracing::debug!("Fetching metadata for {} NFT tokens", tokens.len());

        let mut handles = Vec::new();
        for (contract_address, token_id, token_uri) in tokens {
            let pool = self.pool.clone();
            let client = self.client.clone();
            let provider = self.provider.clone();
            let ipfs_gateway = self.config.ipfs_gateway.clone();
            let retry_attempts = self.config.metadata_retry_attempts;

            handles.push(tokio::spawn(async move {
                // Errors are logged inside fetch_and_store_token_metadata at debug level
                let _ = fetch_and_store_token_metadata(
                    &pool, &client, &provider, &ipfs_gateway,
                    &contract_address, &token_id, token_uri.as_deref(),
                    retry_attempts
                ).await;
            }));

            if handles.len() >= self.config.metadata_fetch_workers as usize {
                for handle in handles.drain(..) {
                    let _ = handle.await;
                }
            }
        }

        for handle in handles {
            let _ = handle.await;
        }

        Ok(true)
    }
}

/// Fetch NFT contract metadata (name, symbol, totalSupply)
async fn fetch_nft_contract_metadata(
    pool: &PgPool,
    provider: &HttpProvider,
    contract_address: &str,
) -> Result<()> {
    let address = Address::from_str(contract_address)?;
    let contract = IERC721Metadata::new(address, provider);

    // Fetch name (optional - some contracts don't implement it)
    let name = contract.name().call().await.ok().map(|r| r._0);

    // Fetch symbol (optional)
    let symbol = contract.symbol().call().await.ok().map(|r| r._0);

    // Fetch totalSupply (optional - ERC-721 doesn't require it)
    let total_supply = contract.totalSupply().call().await.ok().map(|r| r._0.try_into().unwrap_or(0i64));

    sqlx::query(
        "UPDATE nft_contracts SET
            name = COALESCE($2, name),
            symbol = COALESCE($3, symbol),
            total_supply = COALESCE($4, total_supply),
            metadata_fetched = true
         WHERE address = $1"
    )
    .bind(contract_address)
    .bind(name)
    .bind(symbol)
    .bind(total_supply)
    .execute(pool)
    .await?;

    tracing::debug!("Fetched NFT contract metadata for {}", contract_address);
    Ok(())
}

/// Fetch ERC-20 contract metadata (name, symbol, decimals, totalSupply)
async fn fetch_erc20_contract_metadata(
    pool: &PgPool,
    provider: &HttpProvider,
    contract_address: &str,
) -> Result<()> {
    let address = Address::from_str(contract_address)?;
    let contract = IERC20Metadata::new(address, provider);

    // Fetch name
    let name = contract.name().call().await.ok().map(|r| r._0);

    // Fetch symbol
    let symbol = contract.symbol().call().await.ok().map(|r| r._0);

    // Fetch decimals
    let decimals = contract.decimals().call().await.ok().map(|r| r._0 as i16);

    // Fetch totalSupply
    let total_supply = contract.totalSupply().call().await.ok().map(|r| {
        BigDecimal::from_str(&r._0.to_string()).unwrap_or_default()
    });

    sqlx::query(
        "UPDATE erc20_contracts SET
            name = COALESCE($2, name),
            symbol = COALESCE($3, symbol),
            decimals = COALESCE($4, decimals),
            total_supply = COALESCE($5, total_supply),
            metadata_fetched = true
         WHERE address = $1"
    )
    .bind(contract_address)
    .bind(name)
    .bind(symbol)
    .bind(decimals)
    .bind(total_supply)
    .execute(pool)
    .await?;

    tracing::debug!("Fetched ERC-20 contract metadata for {}", contract_address);
    Ok(())
}

async fn fetch_and_store_token_metadata(
    pool: &PgPool,
    client: &reqwest::Client,
    provider: &HttpProvider,
    ipfs_gateway: &str,
    contract_address: &str,
    token_id: &str,
    token_uri: Option<&str>,
    retry_attempts: u32,
) -> Result<()> {
    // If no token_uri, fetch it from the contract
    let uri = match token_uri {
        Some(uri) if !uri.is_empty() => uri.to_string(),
        _ => {
            // Call tokenURI on the contract
            match fetch_token_uri(provider, contract_address, token_id).await {
                Ok(uri) => {
                    // Store the URI in the database for future reference
                    sqlx::query(
                        "UPDATE nft_tokens SET token_uri = $3
                         WHERE contract_address = $1 AND token_id = $2::numeric"
                    )
                    .bind(contract_address)
                    .bind(token_id)
                    .bind(&uri)
                    .execute(pool)
                    .await?;
                    uri
                }
                Err(e) => {
                    tracing::debug!("Failed to fetch tokenURI for {}:{}: {}", contract_address, token_id, e);
                    // Mark as fetched to avoid retrying forever
                    sqlx::query(
                        "UPDATE nft_tokens SET metadata_fetched = true
                         WHERE contract_address = $1 AND token_id = $2::numeric"
                    )
                    .bind(contract_address)
                    .bind(token_id)
                    .execute(pool)
                    .await?;
                    return Ok(());
                }
            }
        }
    };

    // Skip empty URIs
    if uri.is_empty() {
        sqlx::query(
            "UPDATE nft_tokens SET metadata_fetched = true
             WHERE contract_address = $1 AND token_id = $2::numeric"
        )
        .bind(contract_address)
        .bind(token_id)
        .execute(pool)
        .await?;
        return Ok(());
    }

    // Resolve IPFS/Arweave URIs to HTTP
    let fetch_url = resolve_uri(&uri, ipfs_gateway);

    // Fetch metadata with retries
    let mut last_error = None;
    for attempt in 0..retry_attempts {
        match client.get(&fetch_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    // Check content-type to handle direct image URIs
                    let content_type = response
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");

                    // If it's an image, use the URI directly as image_url
                    if content_type.starts_with("image/") {
                        sqlx::query(
                            "UPDATE nft_tokens SET
                                metadata_fetched = true,
                                image_url = $3
                             WHERE contract_address = $1 AND token_id = $2::numeric"
                        )
                        .bind(contract_address)
                        .bind(token_id)
                        .bind(&fetch_url)
                        .execute(pool)
                        .await?;

                        tracing::debug!(
                            "NFT {}:{} has direct image URI ({})",
                            contract_address, token_id, content_type
                        );
                        return Ok(());
                    }

                    // Try to parse as JSON metadata
                    match response.json::<serde_json::Value>().await {
                        Ok(metadata) => {
                            // Extract common fields
                            let name = metadata.get("name").and_then(|v| v.as_str());
                            let image = metadata.get("image")
                                .or_else(|| metadata.get("image_url"))
                                .and_then(|v| v.as_str());

                            // Resolve IPFS image URLs
                            let image_url = image.map(|img| resolve_uri(img, ipfs_gateway));

                            sqlx::query(
                                "UPDATE nft_tokens SET
                                    metadata_fetched = true,
                                    metadata = $3,
                                    name = $4,
                                    image_url = $5
                                 WHERE contract_address = $1 AND token_id = $2::numeric"
                            )
                            .bind(contract_address)
                            .bind(token_id)
                            .bind(&metadata)
                            .bind(name)
                            .bind(image_url)
                            .execute(pool)
                            .await?;

                            return Ok(());
                        }
                        Err(e) => {
                            last_error = Some(format!("JSON parse error: {}", e));
                        }
                    }
                } else {
                    last_error = Some(format!("HTTP {}", response.status()));
                }
            }
            Err(e) => {
                last_error = Some(format!("Request error: {}", e));
            }
        }

        // Exponential backoff
        if attempt < retry_attempts - 1 {
            tokio::time::sleep(Duration::from_millis(1000 * 2u64.pow(attempt))).await;
        }
    }

    // Mark as fetched even on failure (to avoid infinite retries)
    sqlx::query(
        "UPDATE nft_tokens SET metadata_fetched = true
         WHERE contract_address = $1 AND token_id = $2::numeric"
    )
    .bind(contract_address)
    .bind(token_id)
    .execute(pool)
    .await?;

    // Log at debug level since this is often expected (non-standard NFTs)
    tracing::debug!(
        "Failed to fetch metadata for {}:{}: {}",
        contract_address, token_id, last_error.as_deref().unwrap_or("Unknown error")
    );

    Err(anyhow::anyhow!(last_error.unwrap_or_else(|| "Unknown error".to_string())))
}

/// Call tokenURI on an NFT contract
async fn fetch_token_uri(
    provider: &HttpProvider,
    contract_address: &str,
    token_id: &str,
) -> Result<String> {
    let address = Address::from_str(contract_address)?;
    let token_id_u256 = U256::from_str(token_id)?;

    let contract = IERC721Metadata::new(address, provider);
    let uri = contract.tokenURI(token_id_u256).call().await?;

    Ok(uri._0)
}

/// Resolve IPFS, Arweave, and other URI schemes to HTTP URLs
fn resolve_uri(uri: &str, ipfs_gateway: &str) -> String {
    if uri.starts_with("ipfs://") {
        format!("{}{}", ipfs_gateway, &uri[7..])
    } else if uri.starts_with("ar://") {
        format!("https://arweave.net/{}", &uri[5..])
    } else if uri.starts_with("data:") {
        // Data URIs are already complete
        uri.to_string()
    } else {
        uri.to_string()
    }
}
