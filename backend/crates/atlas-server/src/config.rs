use anyhow::{bail, Context, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    // Shared
    pub database_url: String,
    pub rpc_url: String,

    // Indexer pool
    pub indexer_db_max_connections: u32,

    // API pool
    pub api_db_max_connections: u32,

    // Indexer-specific
    pub rpc_requests_per_second: u32,
    pub start_block: u64,
    pub batch_size: u64,
    pub reindex: bool,
    pub ipfs_gateway: String,
    pub metadata_fetch_workers: u32,
    pub metadata_retry_attempts: u32,
    pub fetch_workers: u32,
    pub rpc_batch_size: u32,

    // API-specific
    pub api_host: String,
    pub api_port: u16,
    pub sse_replay_buffer_blocks: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let sse_replay_buffer_blocks: usize = env::var("SSE_REPLAY_BUFFER_BLOCKS")
            .unwrap_or_else(|_| "4096".to_string())
            .parse()
            .context("Invalid SSE_REPLAY_BUFFER_BLOCKS")?;
        if sse_replay_buffer_blocks == 0 || sse_replay_buffer_blocks > 100_000 {
            bail!("SSE_REPLAY_BUFFER_BLOCKS must be between 1 and 100000");
        }

        Ok(Self {
            database_url: env::var("DATABASE_URL").context("DATABASE_URL must be set")?,
            rpc_url: env::var("RPC_URL").context("RPC_URL must be set")?,

            indexer_db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .context("Invalid DB_MAX_CONNECTIONS")?,
            api_db_max_connections: env::var("API_DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .context("Invalid API_DB_MAX_CONNECTIONS")?,

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

            api_host: env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            api_port: env::var("API_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .context("Invalid API_PORT")?,
            sse_replay_buffer_blocks,
        })
    }
}
