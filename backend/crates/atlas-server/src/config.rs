use anyhow::{bail, Context, Result};
use std::env;

const DEFAULT_DA_WORKER_CONCURRENCY: u32 = 50;
const DEFAULT_DA_RPC_REQUESTS_PER_SECOND: u32 = 50;

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

    // DA tracking (optional)
    pub da_tracking_enabled: bool,
    pub evnode_url: Option<String>,
    pub da_worker_concurrency: u32,
    pub da_rpc_requests_per_second: u32,

    // API-specific
    pub api_host: String,
    pub api_port: u16,
    /// If set, restrict CORS to this exact origin. When unset, any origin is allowed
    /// (backwards-compatible default for development / self-hosted deployments).
    pub cors_origin: Option<String>,
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

        let da_tracking_enabled: bool = env::var("ENABLE_DA_TRACKING")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .context("Invalid ENABLE_DA_TRACKING")?;

        let raw_evnode_url = env::var("EVNODE_URL")
            .ok()
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty());

        let evnode_url = if da_tracking_enabled {
            Some(raw_evnode_url.ok_or_else(|| {
                anyhow::anyhow!("EVNODE_URL must be set when ENABLE_DA_TRACKING=true")
            })?)
        } else {
            None
        };

        let da_worker_concurrency = if da_tracking_enabled {
            let value: u32 = env::var("DA_WORKER_CONCURRENCY")
                .unwrap_or_else(|_| DEFAULT_DA_WORKER_CONCURRENCY.to_string())
                .parse()
                .context("Invalid DA_WORKER_CONCURRENCY")?;
            if value == 0 {
                bail!("DA_WORKER_CONCURRENCY must be greater than 0");
            }
            value
        } else {
            DEFAULT_DA_WORKER_CONCURRENCY
        };

        let da_rpc_requests_per_second = if da_tracking_enabled {
            let value: u32 = env::var("DA_RPC_REQUESTS_PER_SECOND")
                .unwrap_or_else(|_| DEFAULT_DA_RPC_REQUESTS_PER_SECOND.to_string())
                .parse()
                .context("Invalid DA_RPC_REQUESTS_PER_SECOND")?;
            if value == 0 {
                bail!("DA_RPC_REQUESTS_PER_SECOND must be greater than 0");
            }
            value
        } else {
            DEFAULT_DA_RPC_REQUESTS_PER_SECOND
        };

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

            da_tracking_enabled,
            evnode_url,
            da_worker_concurrency,
            da_rpc_requests_per_second,

            api_host: env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            api_port: env::var("API_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .context("Invalid API_PORT")?,
            cors_origin: env::var("CORS_ORIGIN").ok(),
            sse_replay_buffer_blocks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_required_env() {
        env::set_var("DATABASE_URL", "postgres://test@localhost/test");
        env::set_var("RPC_URL", "http://localhost:8545");
    }

    fn clear_da_env() {
        env::remove_var("ENABLE_DA_TRACKING");
        env::remove_var("EVNODE_URL");
        env::remove_var("DA_WORKER_CONCURRENCY");
        env::remove_var("DA_RPC_REQUESTS_PER_SECOND");
    }

    #[test]
    fn sse_replay_buffer_validation() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_da_env();

        // Default
        env::remove_var("SSE_REPLAY_BUFFER_BLOCKS");
        assert_eq!(Config::from_env().unwrap().sse_replay_buffer_blocks, 4096);

        // Valid custom value
        env::set_var("SSE_REPLAY_BUFFER_BLOCKS", "12345");
        assert_eq!(Config::from_env().unwrap().sse_replay_buffer_blocks, 12345);

        // Out-of-range (0 and above max)
        for val in ["0", "100001"] {
            env::set_var("SSE_REPLAY_BUFFER_BLOCKS", val);
            let err = Config::from_env().unwrap_err();
            assert!(
                err.to_string().contains("must be between 1 and 100000"),
                "expected range error for {val}"
            );
        }

        // Non-numeric
        env::set_var("SSE_REPLAY_BUFFER_BLOCKS", "abc");
        assert!(Config::from_env()
            .unwrap_err()
            .to_string()
            .contains("Invalid SSE_REPLAY_BUFFER_BLOCKS"));

        env::remove_var("SSE_REPLAY_BUFFER_BLOCKS");
        clear_da_env();
    }

    #[test]
    fn da_tracking_is_disabled_by_default_and_ignores_da_specific_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_da_env();

        env::set_var("EVNODE_URL", "");
        env::set_var("DA_WORKER_CONCURRENCY", "not-a-number");
        env::set_var("DA_RPC_REQUESTS_PER_SECOND", "not-a-number");

        let config = Config::from_env().unwrap();
        assert!(!config.da_tracking_enabled);
        assert!(config.evnode_url.is_none());
        assert_eq!(config.da_worker_concurrency, DEFAULT_DA_WORKER_CONCURRENCY);
        assert_eq!(
            config.da_rpc_requests_per_second,
            DEFAULT_DA_RPC_REQUESTS_PER_SECOND
        );

        clear_da_env();
    }

    #[test]
    fn da_tracking_requires_non_empty_evnode_url_when_enabled() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_da_env();

        env::set_var("ENABLE_DA_TRACKING", "true");
        let err = Config::from_env().unwrap_err();
        assert!(err
            .to_string()
            .contains("EVNODE_URL must be set when ENABLE_DA_TRACKING=true"));

        env::set_var("EVNODE_URL", "   ");
        let err = Config::from_env().unwrap_err();
        assert!(err
            .to_string()
            .contains("EVNODE_URL must be set when ENABLE_DA_TRACKING=true"));

        clear_da_env();
    }

    #[test]
    fn da_tracking_parses_enabled_config() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_da_env();

        env::set_var("ENABLE_DA_TRACKING", "true");
        env::set_var("EVNODE_URL", "http://localhost:7331/");
        env::set_var("DA_WORKER_CONCURRENCY", "12");
        env::set_var("DA_RPC_REQUESTS_PER_SECOND", "34");

        let config = Config::from_env().unwrap();
        assert!(config.da_tracking_enabled);
        assert_eq!(config.evnode_url.as_deref(), Some("http://localhost:7331/"));
        assert_eq!(config.da_worker_concurrency, 12);
        assert_eq!(config.da_rpc_requests_per_second, 34);

        clear_da_env();
    }
}
