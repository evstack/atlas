use thiserror::Error;

#[derive(Error, Debug)]
pub enum AtlasError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Metadata fetch error: {0}")]
    MetadataFetch(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Verification error: {0}")]
    Verification(String),

    #[error("Compilation error: {0}")]
    Compilation(String),

    #[error("Bytecode mismatch: {0}")]
    BytecodeMismatch(String),
}

impl AtlasError {
    pub fn status_code(&self) -> u16 {
        match self {
            AtlasError::NotFound(_) => 404,
            AtlasError::InvalidInput(_) => 400,
            AtlasError::Unauthorized(_) => 401,
            AtlasError::Database(_) | AtlasError::Internal(_) => 500,
            AtlasError::Rpc(_) | AtlasError::MetadataFetch(_) => 502,
            AtlasError::Config(_) => 500,
            AtlasError::Verification(_) | AtlasError::BytecodeMismatch(_) => 400,
            AtlasError::Compilation(_) => 422,
        }
    }
}
