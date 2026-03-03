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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_returns_404() {
        assert_eq!(AtlasError::NotFound("resource".into()).status_code(), 404);
    }

    #[test]
    fn invalid_input_returns_400() {
        assert_eq!(AtlasError::InvalidInput("bad input".into()).status_code(), 400);
    }

    #[test]
    fn unauthorized_returns_401() {
        assert_eq!(AtlasError::Unauthorized("no key".into()).status_code(), 401);
    }

    #[test]
    fn internal_error_returns_500() {
        assert_eq!(AtlasError::Internal("oops".into()).status_code(), 500);
    }

    #[test]
    fn rpc_error_returns_502() {
        assert_eq!(AtlasError::Rpc("timeout".into()).status_code(), 502);
    }

    #[test]
    fn metadata_fetch_returns_502() {
        assert_eq!(AtlasError::MetadataFetch("ipfs down".into()).status_code(), 502);
    }

    #[test]
    fn config_error_returns_500() {
        assert_eq!(AtlasError::Config("missing env".into()).status_code(), 500);
    }

    #[test]
    fn verification_error_returns_400() {
        assert_eq!(AtlasError::Verification("bad source".into()).status_code(), 400);
    }

    #[test]
    fn bytecode_mismatch_returns_400() {
        assert_eq!(AtlasError::BytecodeMismatch("different".into()).status_code(), 400);
    }

    #[test]
    fn compilation_error_returns_422() {
        assert_eq!(AtlasError::Compilation("syntax error".into()).status_code(), 422);
    }
}
