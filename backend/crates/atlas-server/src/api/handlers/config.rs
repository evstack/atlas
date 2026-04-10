use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::api::AppState;

#[derive(Serialize)]
pub struct ChainFeatures {
    pub da_tracking: bool,
}

#[derive(Serialize)]
pub struct FaucetConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_wei: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown_minutes: Option<u64>,
}

#[derive(Serialize)]
pub struct BrandingConfig {
    pub chain_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url_light: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url_dark: Option<String>,
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
    pub features: ChainFeatures,
    pub faucet: FaucetConfig,
}

/// GET /api/config - Returns white-label branding configuration
/// No DB access, no auth — returns static config from environment variables
pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<BrandingConfig> {
    Json(BrandingConfig {
        chain_name: state.chain_name.clone(),
        logo_url: state.chain_logo_url.clone(),
        logo_url_light: state.chain_logo_url_light.clone(),
        logo_url_dark: state.chain_logo_url_dark.clone(),
        accent_color: state.accent_color.clone(),
        background_color_dark: state.background_color_dark.clone(),
        background_color_light: state.background_color_light.clone(),
        success_color: state.success_color.clone(),
        error_color: state.error_color.clone(),
        features: ChainFeatures {
            da_tracking: state.da_tracking_enabled,
        },
        faucet: FaucetConfig {
            enabled: state.faucet.is_some(),
            amount_wei: state.faucet_amount_wei.clone(),
            cooldown_minutes: state.faucet_cooldown_minutes,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branding_config_skips_none_fields() {
        let config = BrandingConfig {
            chain_name: "TestChain".to_string(),
            logo_url: None,
            logo_url_light: None,
            logo_url_dark: None,
            accent_color: Some("#3b82f6".to_string()),
            background_color_dark: None,
            background_color_light: None,
            success_color: None,
            error_color: None,
            features: ChainFeatures { da_tracking: false },
            faucet: FaucetConfig {
                enabled: false,
                amount_wei: None,
                cooldown_minutes: None,
            },
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["chain_name"], "TestChain");
        assert_eq!(json["accent_color"], "#3b82f6");
        assert_eq!(json["features"]["da_tracking"], false);
        assert_eq!(json["faucet"]["enabled"], false);
        assert!(json.get("logo_url").is_none());
        assert!(json.get("logo_url_light").is_none());
        assert!(json.get("logo_url_dark").is_none());
        assert!(json.get("background_color_dark").is_none());
        assert!(json.get("success_color").is_none());
        assert!(json.get("error_color").is_none());
        assert!(json["faucet"].get("amount_wei").is_none());
        assert!(json["faucet"].get("cooldown_minutes").is_none());
    }

    #[test]
    fn branding_config_includes_all_fields_when_set() {
        let config = BrandingConfig {
            chain_name: "MyChain".to_string(),
            logo_url: Some("/branding/logo.svg".to_string()),
            logo_url_light: Some("/branding/logo-light.svg".to_string()),
            logo_url_dark: Some("/branding/logo-dark.svg".to_string()),
            accent_color: Some("#3b82f6".to_string()),
            background_color_dark: Some("#0a0a0f".to_string()),
            background_color_light: Some("#faf5ef".to_string()),
            success_color: Some("#10b981".to_string()),
            error_color: Some("#ef4444".to_string()),
            features: ChainFeatures { da_tracking: true },
            faucet: FaucetConfig {
                enabled: true,
                amount_wei: Some("100000000000000000".to_string()),
                cooldown_minutes: Some(30),
            },
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["chain_name"], "MyChain");
        assert_eq!(json["logo_url"], "/branding/logo.svg");
        assert_eq!(json["logo_url_light"], "/branding/logo-light.svg");
        assert_eq!(json["logo_url_dark"], "/branding/logo-dark.svg");
        assert_eq!(json["accent_color"], "#3b82f6");
        assert_eq!(json["background_color_dark"], "#0a0a0f");
        assert_eq!(json["background_color_light"], "#faf5ef");
        assert_eq!(json["success_color"], "#10b981");
        assert_eq!(json["error_color"], "#ef4444");
        assert_eq!(json["features"]["da_tracking"], true);
        assert_eq!(json["faucet"]["enabled"], true);
        assert_eq!(json["faucet"]["amount_wei"], "100000000000000000");
        assert_eq!(json["faucet"]["cooldown_minutes"], 30);
    }
}
