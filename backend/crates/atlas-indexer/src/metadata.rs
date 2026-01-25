use anyhow::Result;
use sqlx::PgPool;
use std::time::Duration;

use crate::config::Config;

pub struct MetadataFetcher {
    pool: PgPool,
    config: Config,
    client: reqwest::Client,
}

impl MetadataFetcher {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { pool, config, client }
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!("Starting metadata fetcher with {} workers", self.config.metadata_fetch_workers);

        loop {
            // Fetch tokens that need metadata
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
                // No work, sleep
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            // Process tokens concurrently
            let mut handles = Vec::new();
            for (contract_address, token_id, token_uri) in tokens {
                let pool = self.pool.clone();
                let client = self.client.clone();
                let ipfs_gateway = self.config.ipfs_gateway.clone();
                let retry_attempts = self.config.metadata_retry_attempts;

                handles.push(tokio::spawn(async move {
                    if let Err(e) = fetch_and_store_metadata(
                        &pool, &client, &ipfs_gateway,
                        &contract_address, &token_id, token_uri.as_deref(),
                        retry_attempts
                    ).await {
                        tracing::warn!(
                            "Failed to fetch metadata for {}:{}: {}",
                            contract_address, token_id, e
                        );
                    }
                }));

                // Limit concurrent fetches
                if handles.len() >= self.config.metadata_fetch_workers as usize {
                    for handle in handles.drain(..) {
                        let _ = handle.await;
                    }
                }
            }

            // Wait for remaining
            for handle in handles {
                let _ = handle.await;
            }
        }
    }
}

async fn fetch_and_store_metadata(
    pool: &PgPool,
    client: &reqwest::Client,
    ipfs_gateway: &str,
    contract_address: &str,
    token_id: &str,
    token_uri: Option<&str>,
    retry_attempts: u32,
) -> Result<()> {
    // If no token_uri, we need to fetch it from the contract
    let uri = match token_uri {
        Some(uri) => uri.to_string(),
        None => {
            // TODO: Call tokenURI on contract
            // For now, mark as fetched with no metadata
            sqlx::query(
                "UPDATE nft_tokens SET metadata_fetched = true WHERE contract_address = $1 AND token_id = $2::numeric"
            )
            .bind(contract_address)
            .bind(token_id)
            .execute(pool)
            .await?;
            return Ok(());
        }
    };

    // Resolve IPFS URIs
    let fetch_url = if uri.starts_with("ipfs://") {
        format!("{}{}", ipfs_gateway, &uri[7..])
    } else if uri.starts_with("ar://") {
        format!("https://arweave.net/{}", &uri[5..])
    } else {
        uri.clone()
    };

    // Fetch with retries
    let mut last_error = None;
    for attempt in 0..retry_attempts {
        match client.get(&fetch_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(metadata) => {
                            // Extract common fields
                            let name = metadata.get("name").and_then(|v| v.as_str());
                            let image = metadata.get("image")
                                .or_else(|| metadata.get("image_url"))
                                .and_then(|v| v.as_str());

                            // Resolve IPFS image URLs
                            let image_url = image.map(|img| {
                                if img.starts_with("ipfs://") {
                                    format!("{}{}", ipfs_gateway, &img[7..])
                                } else {
                                    img.to_string()
                                }
                            });

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
        "UPDATE nft_tokens SET metadata_fetched = true WHERE contract_address = $1 AND token_id = $2::numeric"
    )
    .bind(contract_address)
    .bind(token_id)
    .execute(pool)
    .await?;

    Err(anyhow::anyhow!(last_error.unwrap_or_else(|| "Unknown error".to_string())))
}
