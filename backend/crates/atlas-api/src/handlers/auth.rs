use crate::AppState;
use atlas_common::AtlasError;
use axum::http::HeaderMap;

pub fn require_admin(headers: &HeaderMap, state: &AppState) -> Result<(), AtlasError> {
    let configured = state
        .admin_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| AtlasError::Unauthorized("Admin API is not enabled".to_string()))?;

    let provided = headers
        .get("x-admin-api-key")
        .and_then(|v| v.to_str().ok())
        .or_else(|| headers.get("x-api-key").and_then(|v| v.to_str().ok()));

    match provided {
        Some(key) if key == configured => Ok(()),
        _ => Err(AtlasError::Unauthorized(
            "Missing or invalid admin API key".to_string(),
        )),
    }
}
