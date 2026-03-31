use alloy::primitives::U256;
use alloy::signers::local::PrivateKeySigner;
use anyhow::{bail, Context, Result};
use std::{env, str::FromStr};

#[cfg(test)]
const DEFAULT_DA_WORKER_CONCURRENCY: u32 = 50;
#[cfg(test)]
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
    pub chain_name: String,

    // Branding / white-label
    pub chain_logo_url: Option<String>,
    pub chain_logo_url_light: Option<String>,
    pub chain_logo_url_dark: Option<String>,
    pub accent_color: Option<String>,
    pub background_color_dark: Option<String>,
    pub background_color_light: Option<String>,
    pub success_color: Option<String>,
    pub error_color: Option<String>,
}

#[derive(Clone)]
pub struct FaucetConfig {
    pub enabled: bool,
    pub private_key: Option<String>,
    pub amount_wei: Option<U256>,
    pub cooldown_minutes: Option<u64>,
}

impl std::fmt::Debug for FaucetConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaucetConfig")
            .field("enabled", &self.enabled)
            .field(
                "private_key",
                &self.private_key.as_ref().map(|_| "[redacted]"),
            )
            .field("amount_wei", &self.amount_wei)
            .field("cooldown_minutes", &self.cooldown_minutes)
            .finish()
    }
}

#[cfg(test)]
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
                anyhow::anyhow!("EVNODE_URL must be set when DA tracking is enabled")
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
            chain_name: env::var("CHAIN_NAME")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Unknown".to_string()),
            chain_logo_url: parse_optional_env(env::var("CHAIN_LOGO_URL").ok()),
            chain_logo_url_light: parse_optional_env(env::var("CHAIN_LOGO_URL_LIGHT").ok()),
            chain_logo_url_dark: parse_optional_env(env::var("CHAIN_LOGO_URL_DARK").ok()),
            accent_color: parse_optional_env(env::var("ACCENT_COLOR").ok()),
            background_color_dark: parse_optional_env(env::var("BACKGROUND_COLOR_DARK").ok()),
            background_color_light: parse_optional_env(env::var("BACKGROUND_COLOR_LIGHT").ok()),
            success_color: parse_optional_env(env::var("SUCCESS_COLOR").ok()),
            error_color: parse_optional_env(env::var("ERROR_COLOR").ok()),
        })
    }
}

#[cfg(test)]
impl FaucetConfig {
    pub fn from_env() -> Result<Self> {
        let enabled = env::var("FAUCET_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .context("Invalid FAUCET_ENABLED")?;

        if !enabled {
            return Ok(Self {
                enabled,
                private_key: None,
                amount_wei: None,
                cooldown_minutes: None,
            });
        }

        let private_key = env::var("FAUCET_PRIVATE_KEY")
            .context("FAUCET_PRIVATE_KEY must be set when FAUCET_ENABLED=true")?;
        PrivateKeySigner::from_str(&private_key).context("Invalid FAUCET_PRIVATE_KEY")?;

        let amount = env::var("FAUCET_AMOUNT")
            .context("FAUCET_AMOUNT must be set when FAUCET_ENABLED=true")?;
        let amount_wei = parse_faucet_amount_to_wei(&amount)?;
        if amount_wei == U256::ZERO {
            bail!("FAUCET_AMOUNT must be greater than 0");
        }

        let cooldown_minutes = env::var("FAUCET_COOLDOWN_MINUTES")
            .context("FAUCET_COOLDOWN_MINUTES must be set when FAUCET_ENABLED=true")?
            .parse::<u64>()
            .context("Invalid FAUCET_COOLDOWN_MINUTES")?;
        if cooldown_minutes == 0 {
            bail!("FAUCET_COOLDOWN_MINUTES must be greater than 0");
        }
        if cooldown_minutes.checked_mul(60).is_none() {
            bail!("FAUCET_COOLDOWN_MINUTES is too large");
        }

        Ok(Self {
            enabled,
            private_key: Some(private_key),
            amount_wei: Some(amount_wei),
            cooldown_minutes: Some(cooldown_minutes),
        })
    }
}

// ── CLI → Config conversion ───────────────────────────────────────────────────

impl Config {
    pub fn from_run_args(args: crate::cli::RunArgs) -> anyhow::Result<Self> {
        let database_url = args.db.url.trim().to_string();
        if database_url.is_empty() {
            bail!("DATABASE_URL must be set");
        }

        let sse_replay_buffer_blocks = args.api.sse_replay_buffer_blocks;
        if sse_replay_buffer_blocks == 0 || sse_replay_buffer_blocks > 100_000 {
            bail!("--atlas.api.sse-replay-buffer-blocks must be between 1 and 100000");
        }

        let da_tracking_enabled = args.da.enabled;

        if da_tracking_enabled && args.da.worker_concurrency == 0 {
            bail!("--atlas.da.worker-concurrency must be greater than 0");
        }
        if da_tracking_enabled && args.da.rpc_requests_per_second == 0 {
            bail!("--atlas.da.rpc-requests-per-second must be greater than 0");
        }

        let evnode_url = if da_tracking_enabled {
            let url = args
                .da
                .evnode_url
                .map(|s: String| s.trim().to_string())
                .filter(|s: &String| !s.is_empty());
            Some(url.ok_or_else(|| {
                anyhow::anyhow!(
                    "--atlas.da.evnode-url (or EVNODE_URL) must be set when DA tracking is enabled"
                )
            })?)
        } else {
            None
        };

        let chain_name = args.chain.name.trim().to_string();
        let chain_name = if chain_name.is_empty() {
            "Unknown".to_string()
        } else {
            chain_name
        };

        Ok(Self {
            database_url,
            rpc_url: args.rpc.url,
            indexer_db_max_connections: args.db.max_connections,
            api_db_max_connections: args.db.api_max_connections,
            rpc_requests_per_second: args.rpc.requests_per_second,
            start_block: args.indexer.start_block,
            batch_size: args.indexer.batch_size,
            reindex: args.indexer.reindex,
            ipfs_gateway: args.indexer.ipfs_gateway,
            metadata_fetch_workers: args.indexer.metadata_fetch_workers,
            metadata_retry_attempts: args.indexer.metadata_retry_attempts,
            fetch_workers: args.indexer.fetch_workers,
            rpc_batch_size: args.rpc.batch_size,
            da_tracking_enabled,
            evnode_url,
            da_worker_concurrency: args.da.worker_concurrency,
            da_rpc_requests_per_second: args.da.rpc_requests_per_second,
            api_host: args.api.host,
            api_port: args.api.port,
            cors_origin: parse_optional_env(args.api.cors_origin),
            sse_replay_buffer_blocks,
            chain_name,
            chain_logo_url: parse_optional_env(args.chain.logo_url),
            chain_logo_url_light: parse_optional_env(args.chain.logo_url_light),
            chain_logo_url_dark: parse_optional_env(args.chain.logo_url_dark),
            accent_color: parse_optional_env(args.branding.accent_color),
            background_color_dark: parse_optional_env(args.branding.background_dark),
            background_color_light: parse_optional_env(args.branding.background_light),
            success_color: parse_optional_env(args.branding.success_color),
            error_color: parse_optional_env(args.branding.error_color),
        })
    }
}

impl FaucetConfig {
    pub fn from_faucet_args(args: &crate::cli::FaucetArgs) -> anyhow::Result<Self> {
        if !args.enabled {
            return Ok(Self {
                enabled: false,
                private_key: None,
                amount_wei: None,
                cooldown_minutes: None,
            });
        }

        let private_key = env::var("FAUCET_PRIVATE_KEY")
            .context("FAUCET_PRIVATE_KEY env var must be set when faucet is enabled")?;
        PrivateKeySigner::from_str(&private_key).context("Invalid FAUCET_PRIVATE_KEY")?;

        let amount_str = args.amount.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "--atlas.faucet.amount (or FAUCET_AMOUNT) must be set when faucet is enabled"
            )
        })?;
        let amount_wei = parse_faucet_amount_to_wei(amount_str)?;
        if amount_wei == U256::ZERO {
            bail!("faucet amount must be greater than 0");
        }

        let cooldown_minutes = args.cooldown_minutes.ok_or_else(|| {
            anyhow::anyhow!(
                "--atlas.faucet.cooldown-minutes (or FAUCET_COOLDOWN_MINUTES) must be set when faucet is enabled"
            )
        })?;
        if cooldown_minutes == 0 {
            bail!("faucet cooldown must be greater than 0");
        }
        if cooldown_minutes.checked_mul(60).is_none() {
            bail!("faucet cooldown is too large");
        }

        Ok(Self {
            enabled: true,
            private_key: Some(private_key),
            amount_wei: Some(amount_wei),
            cooldown_minutes: Some(cooldown_minutes),
        })
    }
}

fn parse_optional_env(val: Option<String>) -> Option<String> {
    val.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn parse_faucet_amount_to_wei(amount: &str) -> Result<U256> {
    let trimmed = amount.trim();
    if trimmed.is_empty() {
        bail!("FAUCET_AMOUNT must not be empty");
    }
    if trimmed.starts_with('-') {
        bail!("FAUCET_AMOUNT must be positive");
    }

    let (whole, fractional) = match trimmed.split_once('.') {
        Some((whole, fractional)) => (whole, fractional),
        None => (trimmed, ""),
    };

    if whole.is_empty() && fractional.is_empty() {
        bail!("FAUCET_AMOUNT must contain digits");
    }
    if !whole.chars().all(|c| c.is_ascii_digit()) || !fractional.chars().all(|c| c.is_ascii_digit())
    {
        bail!("FAUCET_AMOUNT must be a decimal ETH value");
    }
    if fractional.len() > 18 {
        bail!("FAUCET_AMOUNT supports at most 18 decimal places");
    }

    let wei_per_eth = U256::from(1_000_000_000_000_000_000u128);
    let whole_wei = if whole.is_empty() {
        U256::ZERO
    } else {
        U256::from_str_radix(whole, 10).context("Invalid FAUCET_AMOUNT")?
    };

    let fractional_wei = if fractional.is_empty() {
        U256::ZERO
    } else {
        let mut padded = fractional.to_string();
        padded.extend(std::iter::repeat_n('0', 18 - fractional.len()));
        U256::from_str_radix(&padded, 10).context("Invalid FAUCET_AMOUNT")?
    };

    Ok(whole_wei * wei_per_eth + fractional_wei)
}

#[cfg(test)]
mod tests_from_run_args {
    use super::*;
    use crate::cli;

    fn minimal_run_args() -> cli::RunArgs {
        cli::RunArgs {
            db: cli::DatabaseArgs {
                url: "postgres://test@localhost/test".to_string(),
                max_connections: 20,
                api_max_connections: 20,
            },
            rpc: cli::RpcArgs {
                url: "http://localhost:8545".to_string(),
                requests_per_second: 100,
                batch_size: 20,
            },
            api: cli::ApiArgs {
                host: "127.0.0.1".to_string(),
                port: 3000,
                cors_origin: None,
                sse_replay_buffer_blocks: 4096,
            },
            indexer: cli::IndexerArgs {
                start_block: 0,
                batch_size: 100,
                fetch_workers: 10,
                reindex: false,
                ipfs_gateway: "https://ipfs.io/ipfs/".to_string(),
                metadata_fetch_workers: 4,
                metadata_retry_attempts: 3,
            },
            chain: cli::ChainArgs {
                name: "TestChain".to_string(),
                logo_url: None,
                logo_url_light: None,
                logo_url_dark: None,
            },
            da: cli::DaArgs {
                enabled: false,
                evnode_url: None,
                worker_concurrency: 50,
                rpc_requests_per_second: 50,
            },
            faucet: cli::FaucetArgs {
                enabled: false,
                amount: None,
                cooldown_minutes: None,
            },
            branding: cli::BrandingArgs {
                accent_color: None,
                background_dark: None,
                background_light: None,
                success_color: None,
                error_color: None,
            },
            log: cli::LogArgs {
                level: "info".to_string(),
            },
        }
    }

    #[test]
    fn minimal_args_produce_valid_config() {
        let config = Config::from_run_args(minimal_run_args()).unwrap();
        assert_eq!(config.database_url, "postgres://test@localhost/test");
        assert_eq!(config.rpc_url, "http://localhost:8545");
        assert_eq!(config.chain_name, "TestChain");
        assert!(!config.da_tracking_enabled);
    }

    #[test]
    fn chain_name_trimmed_and_defaults_to_unknown_when_blank() {
        let mut args = minimal_run_args();
        args.chain.name = "   ".to_string();
        assert_eq!(Config::from_run_args(args).unwrap().chain_name, "Unknown");
    }

    #[test]
    fn chain_name_surrounding_whitespace_is_trimmed() {
        let mut args = minimal_run_args();
        args.chain.name = "  MyChain  ".to_string();
        assert_eq!(Config::from_run_args(args).unwrap().chain_name, "MyChain");
    }

    #[test]
    fn sse_replay_buffer_zero_is_rejected() {
        let mut args = minimal_run_args();
        args.api.sse_replay_buffer_blocks = 0;
        assert!(Config::from_run_args(args)
            .unwrap_err()
            .to_string()
            .contains("must be between 1 and 100000"));
    }

    #[test]
    fn sse_replay_buffer_above_max_is_rejected() {
        let mut args = minimal_run_args();
        args.api.sse_replay_buffer_blocks = 100_001;
        assert!(Config::from_run_args(args)
            .unwrap_err()
            .to_string()
            .contains("must be between 1 and 100000"));
    }

    #[test]
    fn da_tracking_requires_evnode_url() {
        let mut args = minimal_run_args();
        args.da.enabled = true;
        args.da.evnode_url = None;
        assert!(Config::from_run_args(args)
            .unwrap_err()
            .to_string()
            .contains("evnode-url"));
    }

    #[test]
    fn da_tracking_rejects_blank_evnode_url() {
        let mut args = minimal_run_args();
        args.da.enabled = true;
        args.da.evnode_url = Some("   ".to_string());
        assert!(Config::from_run_args(args)
            .unwrap_err()
            .to_string()
            .contains("evnode-url"));
    }

    #[test]
    fn da_tracking_disabled_does_not_require_evnode_url() {
        let mut args = minimal_run_args();
        args.da.enabled = false;
        args.da.evnode_url = None;
        let config = Config::from_run_args(args).unwrap();
        assert!(config.evnode_url.is_none());
    }

    #[test]
    fn da_worker_concurrency_zero_is_rejected_when_da_enabled() {
        let mut args = minimal_run_args();
        args.da.enabled = true;
        args.da.evnode_url = Some("http://localhost:7331".to_string());
        args.da.worker_concurrency = 0;
        assert!(Config::from_run_args(args).is_err());
    }

    #[test]
    fn branding_blank_strings_become_none() {
        let mut args = minimal_run_args();
        args.chain.logo_url_light = Some("   ".to_string());
        args.branding.success_color = Some("#00ff00".to_string());
        let config = Config::from_run_args(args).unwrap();
        assert!(config.chain_logo_url_light.is_none());
        assert_eq!(config.success_color.as_deref(), Some("#00ff00"));
    }

    #[test]
    fn theme_specific_logo_urls_are_trimmed() {
        let mut args = minimal_run_args();
        args.chain.logo_url_light = Some("  /branding/light.svg  ".to_string());
        args.chain.logo_url_dark = Some("  /branding/dark.svg  ".to_string());

        let config = Config::from_run_args(args).unwrap();

        assert_eq!(
            config.chain_logo_url_light.as_deref(),
            Some("/branding/light.svg")
        );
        assert_eq!(
            config.chain_logo_url_dark.as_deref(),
            Some("/branding/dark.svg")
        );
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

    fn clear_faucet_env() {
        env::remove_var("FAUCET_ENABLED");
        env::remove_var("FAUCET_PRIVATE_KEY");
        env::remove_var("FAUCET_AMOUNT");
        env::remove_var("FAUCET_COOLDOWN_MINUTES");
    }

    fn clear_branding_env() {
        env::remove_var("CHAIN_LOGO_URL");
        env::remove_var("CHAIN_LOGO_URL_LIGHT");
        env::remove_var("CHAIN_LOGO_URL_DARK");
    }

    fn set_valid_faucet_env() {
        env::set_var("FAUCET_ENABLED", "true");
        env::set_var(
            "FAUCET_PRIVATE_KEY",
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        );
        env::set_var("FAUCET_AMOUNT", "1.5");
        env::set_var("FAUCET_COOLDOWN_MINUTES", "30");
    }

    #[test]
    fn chain_name_defaults_to_unknown_when_unset() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_branding_env();
        env::remove_var("CHAIN_NAME");
        assert_eq!(Config::from_env().unwrap().chain_name, "Unknown");
    }

    #[test]
    fn chain_name_defaults_to_unknown_when_empty() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_branding_env();
        env::set_var("CHAIN_NAME", "");
        assert_eq!(Config::from_env().unwrap().chain_name, "Unknown");
        env::remove_var("CHAIN_NAME");
    }

    #[test]
    fn chain_name_defaults_to_unknown_when_whitespace_only() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_branding_env();
        env::set_var("CHAIN_NAME", "   ");
        assert_eq!(Config::from_env().unwrap().chain_name, "Unknown");
        env::remove_var("CHAIN_NAME");
    }

    #[test]
    fn chain_name_uses_provided_value() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_branding_env();
        env::set_var("CHAIN_NAME", "MyChain");
        assert_eq!(Config::from_env().unwrap().chain_name, "MyChain");
        env::remove_var("CHAIN_NAME");
    }

    #[test]
    fn chain_name_trims_surrounding_whitespace() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_branding_env();
        env::set_var("CHAIN_NAME", "  MyChain  ");
        assert_eq!(Config::from_env().unwrap().chain_name, "MyChain");
        env::remove_var("CHAIN_NAME");
    }

    #[test]
    fn theme_specific_logo_urls_are_read_from_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_branding_env();
        env::set_var("CHAIN_LOGO_URL_LIGHT", "  /branding/light.svg  ");
        env::set_var("CHAIN_LOGO_URL_DARK", "  /branding/dark.svg  ");

        let config = Config::from_env().unwrap();

        assert_eq!(
            config.chain_logo_url_light.as_deref(),
            Some("/branding/light.svg")
        );
        assert_eq!(
            config.chain_logo_url_dark.as_deref(),
            Some("/branding/dark.svg")
        );

        clear_branding_env();
    }

    #[test]
    fn optional_env_returns_none_when_unset() {
        assert_eq!(parse_optional_env(None), None);
    }

    #[test]
    fn optional_env_returns_none_when_empty() {
        assert_eq!(parse_optional_env(Some("".to_string())), None);
    }

    #[test]
    fn optional_env_returns_none_when_whitespace_only() {
        assert_eq!(parse_optional_env(Some("   ".to_string())), None);
    }

    #[test]
    fn optional_env_trims_and_returns_value() {
        assert_eq!(
            parse_optional_env(Some("  #dc2626  ".to_string())),
            Some("#dc2626".to_string())
        );
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

        env::set_var("EVNODE_URL", "http://ev-node:7331");
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
            .contains("EVNODE_URL must be set when DA tracking is enabled"));

        env::set_var("EVNODE_URL", "   ");
        let err = Config::from_env().unwrap_err();
        assert!(err
            .to_string()
            .contains("EVNODE_URL must be set when DA tracking is enabled"));

        clear_da_env();
    }

    #[test]
    fn evnode_url_alone_does_not_enable_da_tracking() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        clear_da_env();

        env::set_var("EVNODE_URL", "http://ev-node:7331");

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

    #[test]
    fn faucet_config_defaults_disabled() {
        let _lock = ENV_LOCK.lock().unwrap();
        env::remove_var("FAUCET_ENABLED");
        clear_faucet_env();

        let faucet = FaucetConfig::from_env().unwrap();
        assert!(!faucet.enabled);
        assert!(faucet.private_key.is_none());
        assert!(faucet.amount_wei.is_none());
        assert!(faucet.cooldown_minutes.is_none());
    }

    #[test]
    fn faucet_config_validates_enabled_fields() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_valid_faucet_env();

        let faucet = FaucetConfig::from_env().unwrap();
        assert!(faucet.enabled);
        assert_eq!(faucet.cooldown_minutes, Some(30));
        assert_eq!(
            faucet.amount_wei,
            Some(U256::from(1_500_000_000_000_000_000u128))
        );

        env::set_var("FAUCET_AMOUNT", "0.123456789123456789");
        let faucet = FaucetConfig::from_env().unwrap();
        assert_eq!(
            faucet.amount_wei,
            Some(U256::from(123_456_789_123_456_789u128))
        );
    }

    #[test]
    fn faucet_config_rejects_bad_inputs() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_valid_faucet_env();

        for (key, value, expected) in [
            ("FAUCET_ENABLED", "not-a-bool", "Invalid FAUCET_ENABLED"),
            ("FAUCET_PRIVATE_KEY", "0x1234", "Invalid FAUCET_PRIVATE_KEY"),
            (
                "FAUCET_AMOUNT",
                "abc",
                "FAUCET_AMOUNT must be a decimal ETH value",
            ),
            (
                "FAUCET_AMOUNT",
                "1.0000000000000000001",
                "supports at most 18 decimal places",
            ),
            ("FAUCET_COOLDOWN_MINUTES", "0", "must be greater than 0"),
        ] {
            set_valid_faucet_env();
            env::set_var(key, value);
            let err = FaucetConfig::from_env().unwrap_err();
            assert!(
                err.to_string().contains(expected),
                "expected {expected} for {key}={value}"
            );
        }
    }
}
