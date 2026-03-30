use clap::{Args, Parser, Subcommand};

/// Atlas — EVM blockchain explorer
#[derive(Parser)]
#[command(name = "atlas-server", version, about = "EVM blockchain explorer — indexer + API + DA tracking")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the indexer and API server
    Run(Box<RunArgs>),
    /// Run database migrations and exit
    Migrate(Box<MigrateArgs>),
    /// Validate configuration and test DB/RPC connectivity, then exit
    Check(Box<RunArgs>),
    /// Database utilities
    Db(DbCommand),
}

/// Arguments for the `run` and `check` subcommands
#[derive(Args, Clone)]
pub struct RunArgs {
    #[command(flatten)]
    pub db: DatabaseArgs,
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[command(flatten)]
    pub api: ApiArgs,
    #[command(flatten)]
    pub indexer: IndexerArgs,
    #[command(flatten)]
    pub chain: ChainArgs,
    #[command(flatten)]
    pub da: DaArgs,
    #[command(flatten)]
    pub faucet: FaucetArgs,
    #[command(flatten)]
    pub branding: BrandingArgs,
    #[command(flatten)]
    pub log: LogArgs,
}

#[derive(Args, Clone)]
pub struct MigrateArgs {
    #[command(flatten)]
    pub db: DatabaseArgs,
    #[command(flatten)]
    pub log: LogArgs,
}

// ── Sections ──────────────────────────────────────────────────────────────────

#[derive(Args, Clone)]
#[command(next_help_heading = "Database")]
pub struct DatabaseArgs {
    #[arg(
        id = "db-url",
        long = "atlas.db.url",
        env = "DATABASE_URL",
        value_name = "URL",
        help = "PostgreSQL connection string"
    )]
    pub url: String,

    #[arg(
        long = "atlas.db.max-connections",
        env = "DB_MAX_CONNECTIONS",
        default_value = "20",
        value_name = "N",
        help = "Max connections for the indexer pool"
    )]
    pub max_connections: u32,

    #[arg(
        long = "atlas.db.api-max-connections",
        env = "API_DB_MAX_CONNECTIONS",
        default_value = "20",
        value_name = "N",
        help = "Max connections for the API pool"
    )]
    pub api_max_connections: u32,
}

#[derive(Args, Clone)]
#[command(next_help_heading = "RPC")]
pub struct RpcArgs {
    #[arg(
        id = "rpc-url",
        long = "atlas.rpc.url",
        env = "RPC_URL",
        value_name = "URL",
        help = "Ethereum JSON-RPC endpoint"
    )]
    pub url: String,

    #[arg(
        long = "atlas.rpc.requests-per-second",
        env = "RPC_REQUESTS_PER_SECOND",
        default_value = "100",
        value_name = "N",
        help = "Max RPC requests per second"
    )]
    pub requests_per_second: u32,

    #[arg(
        id = "rpc-batch-size",
        long = "atlas.rpc.batch-size",
        env = "RPC_BATCH_SIZE",
        default_value = "20",
        value_name = "N",
        help = "Number of blocks fetched per RPC batch call"
    )]
    pub batch_size: u32,
}

#[derive(Args, Clone)]
#[command(next_help_heading = "API")]
pub struct ApiArgs {
    #[arg(
        long = "atlas.api.host",
        env = "API_HOST",
        default_value = "127.0.0.1",
        value_name = "HOST",
        help = "Host address for the API server"
    )]
    pub host: String,

    #[arg(
        long = "atlas.api.port",
        env = "API_PORT",
        default_value = "3000",
        value_name = "PORT",
        help = "Port for the API server"
    )]
    pub port: u16,

    #[arg(
        long = "atlas.api.cors-origin",
        env = "CORS_ORIGIN",
        value_name = "ORIGIN",
        help = "Restrict CORS to this origin (unset = allow all)"
    )]
    pub cors_origin: Option<String>,

    #[arg(
        long = "atlas.api.sse-replay-buffer-blocks",
        env = "SSE_REPLAY_BUFFER_BLOCKS",
        default_value = "4096",
        value_name = "N",
        help = "Number of recent blocks kept in the SSE replay buffer [1–100000]"
    )]
    pub sse_replay_buffer_blocks: usize,
}

#[derive(Args, Clone)]
#[command(next_help_heading = "Indexer")]
pub struct IndexerArgs {
    #[arg(
        long = "atlas.indexer.start-block",
        env = "START_BLOCK",
        default_value = "0",
        value_name = "N",
        help = "Block number to start indexing from"
    )]
    pub start_block: u64,

    #[arg(
        id = "indexer-batch-size",
        long = "atlas.indexer.batch-size",
        env = "BATCH_SIZE",
        default_value = "100",
        value_name = "N",
        help = "Number of blocks written per DB batch"
    )]
    pub batch_size: u64,

    #[arg(
        long = "atlas.indexer.fetch-workers",
        env = "FETCH_WORKERS",
        default_value = "10",
        value_name = "N",
        help = "Number of concurrent block-fetch workers"
    )]
    pub fetch_workers: u32,

    #[arg(
        long = "atlas.indexer.reindex",
        env = "REINDEX",
        default_value_t = false,
        help = "Wipe indexed data and re-index from start-block"
    )]
    pub reindex: bool,

    #[arg(
        long = "atlas.indexer.ipfs-gateway",
        env = "IPFS_GATEWAY",
        default_value = "https://ipfs.io/ipfs/",
        value_name = "URL",
        help = "IPFS gateway used for token metadata fetching"
    )]
    pub ipfs_gateway: String,

    #[arg(
        long = "atlas.indexer.metadata-fetch-workers",
        env = "METADATA_FETCH_WORKERS",
        default_value = "4",
        value_name = "N",
        help = "Number of concurrent metadata-fetch workers"
    )]
    pub metadata_fetch_workers: u32,

    #[arg(
        long = "atlas.indexer.metadata-retry-attempts",
        env = "METADATA_RETRY_ATTEMPTS",
        default_value = "3",
        value_name = "N",
        help = "Max retry attempts for metadata fetches"
    )]
    pub metadata_retry_attempts: u32,
}

#[derive(Args, Clone)]
#[command(next_help_heading = "Chain")]
pub struct ChainArgs {
    #[arg(
        long = "atlas.chain.name",
        env = "CHAIN_NAME",
        default_value = "Unknown",
        value_name = "NAME",
        help = "Human-readable chain name shown in the UI"
    )]
    pub name: String,

    #[arg(
        long = "atlas.chain.logo-url",
        env = "CHAIN_LOGO_URL",
        value_name = "URL",
        help = "URL to the chain logo image"
    )]
    pub logo_url: Option<String>,
}

#[derive(Args, Clone)]
#[command(next_help_heading = "DA Tracking")]
pub struct DaArgs {
    #[arg(
        id = "da-enabled",
        long = "atlas.da.enabled",
        env = "ENABLE_DA_TRACKING",
        default_value_t = false,
        help = "Enable Celestia DA inclusion tracking"
    )]
    pub enabled: bool,

    #[arg(
        long = "atlas.da.evnode-url",
        env = "EVNODE_URL",
        value_name = "URL",
        help = "ev-node gRPC endpoint (required when DA tracking is enabled)"
    )]
    pub evnode_url: Option<String>,

    #[arg(
        long = "atlas.da.worker-concurrency",
        env = "DA_WORKER_CONCURRENCY",
        default_value = "50",
        value_name = "N",
        help = "Number of concurrent DA worker tasks"
    )]
    pub worker_concurrency: u32,

    #[arg(
        long = "atlas.da.rpc-requests-per-second",
        env = "DA_RPC_REQUESTS_PER_SECOND",
        default_value = "50",
        value_name = "N",
        help = "Max DA RPC requests per second"
    )]
    pub rpc_requests_per_second: u32,
}

#[derive(Args, Clone)]
#[command(next_help_heading = "Faucet")]
pub struct FaucetArgs {
    #[arg(
        id = "faucet-enabled",
        long = "atlas.faucet.enabled",
        env = "FAUCET_ENABLED",
        default_value_t = false,
        help = "Enable the token faucet endpoint"
    )]
    pub enabled: bool,

    #[arg(
        long = "atlas.faucet.amount",
        env = "FAUCET_AMOUNT",
        value_name = "ETH",
        help = "Amount of ETH dispensed per faucet request (e.g. 0.1). FAUCET_PRIVATE_KEY must be set via env"
    )]
    pub amount: Option<String>,

    #[arg(
        long = "atlas.faucet.cooldown-minutes",
        env = "FAUCET_COOLDOWN_MINUTES",
        value_name = "MINS",
        help = "Cooldown period in minutes between faucet requests per address"
    )]
    pub cooldown_minutes: Option<u64>,
    // FAUCET_PRIVATE_KEY is intentionally env-only (security: never pass secrets as CLI flags)
}

#[derive(Args, Clone)]
#[command(next_help_heading = "Branding")]
pub struct BrandingArgs {
    #[arg(
        long = "atlas.branding.accent-color",
        env = "ACCENT_COLOR",
        value_name = "HEX",
        help = "UI accent color (e.g. #3b82f6)"
    )]
    pub accent_color: Option<String>,

    #[arg(
        long = "atlas.branding.background-dark",
        env = "BACKGROUND_COLOR_DARK",
        value_name = "HEX",
        help = "Dark mode background color"
    )]
    pub background_dark: Option<String>,

    #[arg(
        long = "atlas.branding.background-light",
        env = "BACKGROUND_COLOR_LIGHT",
        value_name = "HEX",
        help = "Light mode background color"
    )]
    pub background_light: Option<String>,

    #[arg(
        long = "atlas.branding.success-color",
        env = "SUCCESS_COLOR",
        value_name = "HEX",
        help = "Success state color"
    )]
    pub success_color: Option<String>,

    #[arg(
        long = "atlas.branding.error-color",
        env = "ERROR_COLOR",
        value_name = "HEX",
        help = "Error state color"
    )]
    pub error_color: Option<String>,
}

#[derive(Args, Clone)]
#[command(next_help_heading = "Logging")]
pub struct LogArgs {
    #[arg(
        long = "atlas.log.level",
        env = "RUST_LOG",
        default_value = "atlas_server=info,tower_http=debug,sqlx=warn",
        value_name = "FILTER",
        help = "Log filter directive (e.g. info, atlas_server=debug)"
    )]
    pub level: String,
}

// ── db subcommand ─────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct DbCommand {
    #[command(subcommand)]
    pub command: DbSubcommand,
}

#[derive(Subcommand)]
pub enum DbSubcommand {
    /// Dump the database to a file using pg_dump
    Dump {
        /// Destination file path (use - for stdout)
        #[arg(value_name = "OUTPUT")]
        output: String,

        #[arg(long = "atlas.db.url", env = "DATABASE_URL", value_name = "URL")]
        db_url: String,
    },
    /// Restore the database from a pg_dump file
    Restore {
        /// Source file path (use - for stdin)
        #[arg(value_name = "INPUT")]
        input: String,

        #[arg(long = "atlas.db.url", env = "DATABASE_URL", value_name = "URL")]
        db_url: String,
    },
    /// Drop all indexed data, keeping schema and migrations intact (requires --confirm)
    Reset {
        /// Required to confirm the destructive operation
        #[arg(long)]
        confirm: bool,

        #[arg(long = "atlas.db.url", env = "DATABASE_URL", value_name = "URL")]
        db_url: String,
    },
}
