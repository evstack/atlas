use alloy::primitives::U256;
use alloy::signers::local::PrivateKeySigner;
use anyhow::{bail, Context, Result};
use std::{env, str::FromStr};

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
    /// If set, restrict CORS to this exact origin. When unset, any origin is allowed
    /// (backwards-compatible default for development / self-hosted deployments).
    pub cors_origin: Option<String>,
    pub sse_replay_buffer_blocks: usize,
    pub chain_name: String,

    // Branding / white-label
    pub chain_logo_url: Option<String>,
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
            cors_origin: env::var("CORS_ORIGIN").ok(),
            sse_replay_buffer_blocks,
            chain_name: env::var("CHAIN_NAME")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Unknown".to_string()),
            chain_logo_url: parse_optional_env(env::var("CHAIN_LOGO_URL").ok()),
            accent_color: parse_optional_env(env::var("ACCENT_COLOR").ok()),
            background_color_dark: parse_optional_env(env::var("BACKGROUND_COLOR_DARK").ok()),
            background_color_light: parse_optional_env(env::var("BACKGROUND_COLOR_LIGHT").ok()),
            success_color: parse_optional_env(env::var("SUCCESS_COLOR").ok()),
            error_color: parse_optional_env(env::var("ERROR_COLOR").ok()),
        })
    }
}

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
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_required_env() {
        env::set_var("DATABASE_URL", "postgres://test@localhost/test");
        env::set_var("RPC_URL", "http://localhost:8545");
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
        env::remove_var("CHAIN_NAME");
        assert_eq!(Config::from_env().unwrap().chain_name, "Unknown");
    }

    #[test]
    fn chain_name_defaults_to_unknown_when_empty() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        env::set_var("CHAIN_NAME", "");
        assert_eq!(Config::from_env().unwrap().chain_name, "Unknown");
        env::remove_var("CHAIN_NAME");
    }

    #[test]
    fn chain_name_defaults_to_unknown_when_whitespace_only() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        env::set_var("CHAIN_NAME", "   ");
        assert_eq!(Config::from_env().unwrap().chain_name, "Unknown");
        env::remove_var("CHAIN_NAME");
    }

    #[test]
    fn chain_name_uses_provided_value() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_required_env();
        env::set_var("CHAIN_NAME", "MyChain");
        assert_eq!(Config::from_env().unwrap().chain_name, "MyChain");
        env::remove_var("CHAIN_NAME");
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
    }

    #[test]
    fn faucet_config_defaults_disabled() {
        let _lock = ENV_LOCK.lock().unwrap();
        env::remove_var("FAUCET_ENABLED");
        env::remove_var("FAUCET_PRIVATE_KEY");
        env::remove_var("FAUCET_AMOUNT");
        env::remove_var("FAUCET_COOLDOWN_MINUTES");

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
