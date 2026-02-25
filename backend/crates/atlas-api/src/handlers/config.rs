use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::AppState;

#[derive(Serialize)]
pub struct BrandingConfig {
    pub chain_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color_dark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color_light: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_color: Option<String>,
}

/// GET /api/config - Returns white-label branding configuration
/// No DB access, no auth — returns static config from environment variables
pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<BrandingConfig> {
    Json(BrandingConfig {
        chain_name: state.chain_name.clone(),
        logo_url: state.chain_logo_url.clone(),
        accent_color: state.accent_color.clone(),
        background_color_dark: state.background_color_dark.clone(),
        background_color_light: state.background_color_light.clone(),
        success_color: state.success_color.clone(),
        error_color: state.error_color.clone(),
    })
}
