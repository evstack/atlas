use anyhow::{Context, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub db_max_connections: u32,
    pub rpc_url: String,
    pub rpc_requests_per_second: u32,
    pub start_block: u64,
    pub batch_size: u64,
    pub reindex: bool,
    pub ipfs_gateway: String,
    pub metadata_fetch_workers: u32,
    pub metadata_retry_attempts: u32,
    pub fetch_workers: u32,
    pub rpc_batch_size: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL must be set")?,
            db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .context("Invalid DB_MAX_CONNECTIONS")?,
            rpc_url: env::var("RPC_URL")
                .context("RPC_URL must be set")?,
            rpc_requests_per_second: env::var("RPC_REQUESTS_PER_SECOND")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .context("Invalid RPC_REQUESTS_PER_SECOND")?,
            start_block: env::var("START_BLOCK")
                .unwrap_or_else(|_| "0".to_string())
                .parse()
                .context("Invalid START_BLOCK")?,
            batch_size: env::var("BATCH_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .context("Invalid BATCH_SIZE")?,
            reindex: env::var("REINDEX")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .context("Invalid REINDEX")?,
            ipfs_gateway: env::var("IPFS_GATEWAY")
                .unwrap_or_else(|_| "https://ipfs.io/ipfs/".to_string()),
            metadata_fetch_workers: env::var("METADATA_FETCH_WORKERS")
                .unwrap_or_else(|_| "4".to_string())
                .parse()
                .context("Invalid METADATA_FETCH_WORKERS")?,
            metadata_retry_attempts: env::var("METADATA_RETRY_ATTEMPTS")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .context("Invalid METADATA_RETRY_ATTEMPTS")?,
            fetch_workers: env::var("FETCH_WORKERS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .context("Invalid FETCH_WORKERS")?,
            rpc_batch_size: env::var("RPC_BATCH_SIZE")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .context("Invalid RPC_BATCH_SIZE")?,
        })
    }
}
