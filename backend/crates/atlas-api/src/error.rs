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
        let status = StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = Json(json!({
            "error": self.0.to_string()
        }));
        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
