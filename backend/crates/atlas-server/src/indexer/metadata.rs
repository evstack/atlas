use alloy::{
    network::Ethereum,
    primitives::{Address, U256},
    providers::RootProvider,
    sol,
};
use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;
use std::{str::FromStr, sync::Arc, time::Duration};

use crate::config::Config;
use crate::metrics::Metrics;
use crate::nft_metadata::{
    self, FetchErrorKind, FetchedMetadata, RetryDecision, SsrfSafeResolver, NFT_METADATA_FETCHED,
    NFT_METADATA_PENDING, NFT_METADATA_PERMANENT_ERROR, NFT_METADATA_RETRYABLE_ERROR,
};

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
    }
}

type HttpProvider = RootProvider<Ethereum>;

pub struct MetadataFetcher {
    pool: PgPool,
    config: Config,
    client: reqwest::Client,
    provider: Arc<HttpProvider>,
    metrics: Metrics,
}

impl MetadataFetcher {
    pub fn new(pool: PgPool, config: Config, metrics: Metrics) -> Result<Self> {
        let client = build_metadata_client()?;

        let provider = Arc::new(RootProvider::new_http(config.rpc_url.parse()?));

        Ok(Self {
            pool,
            config,
            client,
            provider,
            metrics,
        })
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!(
            workers = self.config.metadata_fetch_workers,
            "starting metadata fetcher"
        );

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
            "SELECT address FROM nft_contracts WHERE metadata_fetched = false LIMIT $1",
        )
        .bind(self.config.metadata_fetch_workers as i32 * 5)
        .fetch_all(&self.pool)
        .await?;

        if contracts.is_empty() {
            return Ok(false);
        }

        tracing::debug!(count = contracts.len(), "fetching NFT contract metadata");

        let mut handles = Vec::new();
        for (address,) in contracts {
            let pool = self.pool.clone();
            let provider = self.provider.clone();
            let m = self.metrics.clone();

            handles.push(tokio::spawn(async move {
                match fetch_nft_contract_metadata(&pool, &provider, &address).await {
                    Ok(()) => {
                        m.record_metadata_contract_fetched("nft");
                    }
                    Err(e) => {
                        m.record_metadata_error("nft");
                        m.error("metadata", "metadata_fetch");
                        tracing::debug!(
                            address = %address,
                            error = %e,
                            "failed to fetch NFT contract metadata"
                        );
                        // Mark as fetched to avoid infinite retries
                        let _ = sqlx::query(
                            "UPDATE nft_contracts SET metadata_fetched = true WHERE address = $1",
                        )
                        .bind(&address)
                        .execute(&pool)
                        .await;
                    }
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
            "SELECT address FROM erc20_contracts WHERE metadata_fetched = false LIMIT $1",
        )
        .bind(self.config.metadata_fetch_workers as i32 * 5)
        .fetch_all(&self.pool)
        .await?;

        if contracts.is_empty() {
            return Ok(false);
        }

        tracing::debug!(count = contracts.len(), "fetching ERC-20 contract metadata");

        let mut handles = Vec::new();
        for (address,) in contracts {
            let pool = self.pool.clone();
            let provider = self.provider.clone();
            let m = self.metrics.clone();

            handles.push(tokio::spawn(async move {
                match fetch_erc20_contract_metadata(&pool, &provider, &address).await {
                    Ok(()) => {
                        m.record_metadata_contract_fetched("erc20");
                    }
                    Err(e) => {
                        m.record_metadata_error("erc20");
                        m.error("metadata", "metadata_fetch");
                        tracing::debug!(
                            address = %address,
                            error = %e,
                            "failed to fetch ERC-20 contract metadata"
                        );
                        // Mark as fetched to avoid infinite retries
                        let _ = sqlx::query(
                            "UPDATE erc20_contracts SET metadata_fetched = true WHERE address = $1",
                        )
                        .bind(&address)
                        .execute(&pool)
                        .await;
                    }
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
        let tokens: Vec<(String, String, Option<String>, i32)> = sqlx::query_as(
            "SELECT contract_address, token_id::text, token_uri, metadata_retry_count
             FROM nft_tokens
             WHERE metadata_status = $1
                OR (metadata_status = $2 AND next_retry_at <= NOW())
             ORDER BY
                CASE WHEN metadata_status = $2 THEN 0 ELSE 1 END ASC,
                next_retry_at ASC NULLS LAST,
                last_transfer_block DESC
             LIMIT $3",
        )
        .bind(NFT_METADATA_PENDING)
        .bind(NFT_METADATA_RETRYABLE_ERROR)
        .bind(self.config.metadata_fetch_workers as i32 * 10)
        .fetch_all(&self.pool)
        .await?;

        if tokens.is_empty() {
            return Ok(false);
        }

        tracing::debug!(count = tokens.len(), "fetching NFT token metadata");

        let mut handles = Vec::new();
        for (contract_address, token_id, token_uri, retry_count) in tokens {
            let pool = self.pool.clone();
            let client = self.client.clone();
            let provider = self.provider.clone();
            let ipfs_gateway = self.config.ipfs_gateway.clone();
            let retry_attempts = self.config.metadata_retry_attempts;
            let m = self.metrics.clone();

            handles.push(tokio::spawn(async move {
                match fetch_and_store_token_metadata(
                    &pool,
                    &client,
                    &provider,
                    &ipfs_gateway,
                    (&contract_address, &token_id),
                    token_uri.as_deref(),
                    retry_count,
                    retry_attempts,
                )
                .await
                {
                    Ok(true) => {
                        m.record_metadata_token_fetched();
                    }
                    Ok(false) | Err(_) => {
                        m.record_metadata_error("token");
                        m.error("metadata", "metadata_fetch");
                    }
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
}

fn build_metadata_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("atlas-server/0.1.0")
        .dns_resolver(Arc::new(SsrfSafeResolver))
        .build()?)
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
    let name = contract.name().call().await.ok();

    // Fetch symbol (optional)
    let symbol = contract.symbol().call().await.ok();

    // Fetch totalSupply (optional - ERC-721 doesn't require it)
    let total_supply = contract
        .totalSupply()
        .call()
        .await
        .ok()
        .map(|r| r.try_into().unwrap_or(0i64));

    sqlx::query(
        "UPDATE nft_contracts SET
            name = COALESCE($2, name),
            symbol = COALESCE($3, symbol),
            total_supply = COALESCE($4, total_supply),
            metadata_fetched = true
         WHERE address = $1",
    )
    .bind(contract_address)
    .bind(name)
    .bind(symbol)
    .bind(total_supply)
    .execute(pool)
    .await?;

    tracing::debug!(address = %contract_address, "fetched NFT contract metadata");
    Ok(())
}

/// Fetch ERC-20 contract metadata (name, symbol, decimals)
async fn fetch_erc20_contract_metadata(
    pool: &PgPool,
    provider: &HttpProvider,
    contract_address: &str,
) -> Result<()> {
    let address = Address::from_str(contract_address)?;
    let contract = IERC20Metadata::new(address, provider);

    // Fetch name
    let name = contract.name().call().await.ok();

    // Fetch symbol
    let symbol = contract.symbol().call().await.ok();

    // Fetch decimals
    let decimals = contract.decimals().call().await.ok().map(|r| r as i16);

    sqlx::query(
        "UPDATE erc20_contracts SET
            name = COALESCE($2, name),
            symbol = COALESCE($3, symbol),
            decimals = COALESCE($4, decimals),
            metadata_fetched = true
         WHERE address = $1",
    )
    .bind(contract_address)
    .bind(name)
    .bind(symbol)
    .bind(decimals)
    .execute(pool)
    .await?;

    tracing::debug!(address = %contract_address, "fetched ERC-20 contract metadata");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn fetch_and_store_token_metadata(
    pool: &PgPool,
    client: &reqwest::Client,
    provider: &HttpProvider,
    ipfs_gateway: &str,
    token_key: (&str, &str),
    token_uri: Option<&str>,
    metadata_retry_count: i32,
    retry_attempts: u32,
) -> Result<bool> {
    let (contract_address, token_id) = token_key;
    let now = Utc::now();

    sqlx::query(
        "UPDATE nft_tokens
         SET last_metadata_attempted_at = $3
         WHERE contract_address = $1 AND token_id = $2::numeric",
    )
    .bind(contract_address)
    .bind(token_id)
    .bind(now)
    .execute(pool)
    .await?;

    let uri = match token_uri {
        Some(uri) if !uri.is_empty() => uri.to_string(),
        _ => match fetch_token_uri(provider, contract_address, token_id).await {
            Ok(uri) => {
                sqlx::query(
                    "UPDATE nft_tokens SET token_uri = $3
                         WHERE contract_address = $1 AND token_id = $2::numeric",
                )
                .bind(contract_address)
                .bind(token_id)
                .bind(&uri)
                .execute(pool)
                .await?;
                uri
            }
            Err(_) => {
                tracing::debug!(
                    contract = %contract_address,
                    token_id = %token_id,
                    "failed to fetch tokenURI"
                );
                persist_retryable_failure(
                    pool,
                    contract_address,
                    token_id,
                    metadata_retry_count + 1,
                    retry_attempts,
                    "token_uri_fetch_error",
                    now,
                )
                .await?;
                return Ok(false);
            }
        },
    };

    if uri.is_empty() {
        persist_permanent_failure(
            pool,
            contract_address,
            token_id,
            metadata_retry_count,
            "missing_token_uri",
            now,
        )
        .await?;
        return Ok(false);
    }

    match nft_metadata::fetch_metadata(client, &uri, ipfs_gateway).await {
        Ok(FetchedMetadata::DirectImage { image_url }) => {
            sqlx::query(
                "UPDATE nft_tokens SET
                    metadata_status = $3,
                    metadata_retry_count = 0,
                    next_retry_at = NULL,
                    last_metadata_error = NULL,
                    metadata = NULL,
                    image_url = $4,
                    metadata_updated_at = $5
                 WHERE contract_address = $1 AND token_id = $2::numeric",
            )
            .bind(contract_address)
            .bind(token_id)
            .bind(NFT_METADATA_FETCHED)
            .bind(image_url)
            .bind(now)
            .execute(pool)
            .await?;

            Ok(true)
        }
        Ok(FetchedMetadata::Json {
            metadata,
            extracted,
        }) => {
            sqlx::query(
                "UPDATE nft_tokens SET
                    metadata_status = $3,
                    metadata_retry_count = 0,
                    next_retry_at = NULL,
                    last_metadata_error = NULL,
                    metadata = $4,
                    name = $5,
                    image_url = $6,
                    metadata_updated_at = $7
                 WHERE contract_address = $1 AND token_id = $2::numeric",
            )
            .bind(contract_address)
            .bind(token_id)
            .bind(NFT_METADATA_FETCHED)
            .bind(&metadata)
            .bind(extracted.name)
            .bind(extracted.image_url)
            .bind(now)
            .execute(pool)
            .await?;

            Ok(true)
        }
        Err(error) => {
            tracing::debug!(
                contract = %contract_address,
                token_id = %token_id,
                error = %error.code,
                "failed to fetch token metadata"
            );

            match error.kind {
                FetchErrorKind::Retryable => {
                    persist_retryable_failure(
                        pool,
                        contract_address,
                        token_id,
                        metadata_retry_count + 1,
                        retry_attempts,
                        &error.code,
                        now,
                    )
                    .await?;
                }
                FetchErrorKind::Permanent => {
                    persist_permanent_failure(
                        pool,
                        contract_address,
                        token_id,
                        metadata_retry_count,
                        &error.code,
                        now,
                    )
                    .await?;
                }
            }

            Ok(false)
        }
    }
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

    Ok(uri)
}

async fn persist_retryable_failure(
    pool: &PgPool,
    contract_address: &str,
    token_id: &str,
    retry_count: i32,
    retry_attempts: u32,
    error_code: &str,
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    match nft_metadata::schedule_retry(retry_count, retry_attempts, now) {
        RetryDecision::RetryAt(next_retry_at) => {
            sqlx::query(
                "UPDATE nft_tokens SET
                    metadata_status = $3,
                    metadata_retry_count = $4,
                    next_retry_at = $5,
                    last_metadata_error = $6
                 WHERE contract_address = $1 AND token_id = $2::numeric",
            )
            .bind(contract_address)
            .bind(token_id)
            .bind(NFT_METADATA_RETRYABLE_ERROR)
            .bind(retry_count)
            .bind(next_retry_at)
            .bind(error_code)
            .execute(pool)
            .await?;
        }
        RetryDecision::PermanentError => {
            persist_permanent_failure(
                pool,
                contract_address,
                token_id,
                retry_count,
                error_code,
                now,
            )
            .await?;
        }
    }

    Ok(())
}

async fn persist_permanent_failure(
    pool: &PgPool,
    contract_address: &str,
    token_id: &str,
    retry_count: i32,
    error_code: &str,
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE nft_tokens SET
            metadata_status = $3,
            metadata_retry_count = $4,
            next_retry_at = NULL,
            last_metadata_error = $5,
            metadata_updated_at = $6
         WHERE contract_address = $1 AND token_id = $2::numeric",
    )
    .bind(contract_address)
    .bind(token_id)
    .bind(NFT_METADATA_PERMANENT_ERROR)
    .bind(retry_count)
    .bind(error_code)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::build_metadata_client;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn metadata_client_does_not_follow_redirects() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");

        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.expect("accept connection");
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await.expect("read request");
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 302 Found\r\nLocation: http://{addr}/followed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                        .as_bytes(),
                    )
                    .await
                    .expect("write redirect response");
            }
        });

        let client = build_metadata_client().expect("build metadata client");
        let response = client
            .get(format!("http://{addr}/initial"))
            .send()
            .await
            .expect("send request");

        assert_eq!(response.status(), reqwest::StatusCode::FOUND);

        server.abort();
    }
}
