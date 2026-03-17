use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::ops::Deref;

use atlas_common::AtlasError;

/// Newtype wrapper for AtlasError to implement IntoResponse
/// (orphan rule prevents implementing external trait on external type)
pub struct ApiError(pub AtlasError);

impl From<AtlasError> for ApiError {
    fn from(err: AtlasError) -> Self {
        ApiError(err)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        ApiError(AtlasError::Database(err))
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError(AtlasError::Internal(err.to_string()))
    }
}

impl From<alloy::transports::RpcError<alloy::transports::TransportErrorKind>> for ApiError {
    fn from(err: alloy::transports::RpcError<alloy::transports::TransportErrorKind>) -> Self {
        ApiError(AtlasError::Rpc(err.to_string()))
    }
}

impl Deref for ApiError {
    type Target = AtlasError;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        use atlas_common::AtlasError;

        let status =
            StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        // Determine the client-facing message based on error type.
        // Internal details are logged server-side to avoid leaking stack traces or
        // database internals to callers.
        let client_message = match &self.0 {
            // Safe to surface: meaningful to the caller
            AtlasError::NotFound(msg) => msg.clone(),
            AtlasError::InvalidInput(msg) => msg.clone(),
            AtlasError::Validation(msg) => msg.clone(),
            AtlasError::Unauthorized(msg) => msg.clone(),
            AtlasError::Verification(msg) => msg.clone(),
            AtlasError::BytecodeMismatch(msg) => msg.clone(),
            AtlasError::Compilation(msg) => msg.clone(),
            // Opaque: log full detail, return generic message
            AtlasError::Database(inner) => {
                tracing::error!(error = %inner, "Database error");
                "Internal server error".to_string()
            }
            AtlasError::Internal(inner) => {
                tracing::error!(error = %inner, "Internal error");
                "Internal server error".to_string()
            }
            AtlasError::Config(inner) => {
                tracing::error!(error = %inner, "Configuration error");
                "Internal server error".to_string()
            }
            AtlasError::Rpc(inner) => {
                tracing::error!(error = %inner, "RPC error");
                "Service unavailable".to_string()
            }
            AtlasError::MetadataFetch(inner) => {
                tracing::error!(error = %inner, "Metadata fetch error");
                "Service unavailable".to_string()
            }
        };

        let body = Json(json!({ "error": client_message }));
        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
