use alloy::primitives::Address;
use axum::{extract::State, http::HeaderMap, Json};
use serde::Deserialize;
use std::{net::IpAddr, str::FromStr, sync::Arc};

use atlas_common::AtlasError;

use crate::api::error::ApiResult;
use crate::api::AppState;

#[derive(Debug, Deserialize)]
pub struct FaucetRequest {
    pub address: String,
}

pub async fn get_faucet_info(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<crate::faucet::FaucetInfo>> {
    let faucet = state
        .faucet
        .as_ref()
        .ok_or_else(|| AtlasError::NotFound("Faucet is disabled".to_string()))?;

    Ok(Json(faucet.info().await?))
}

pub async fn request_faucet(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<FaucetRequest>,
) -> ApiResult<Json<crate::faucet::FaucetTxResponse>> {
    let faucet = state
        .faucet
        .as_ref()
        .ok_or_else(|| AtlasError::NotFound("Faucet is disabled".to_string()))?;

    let recipient: Address = request
        .address
        .parse()
        .map_err(|_| AtlasError::InvalidInput("Invalid faucet address".to_string()))?;
    let client_ip = extract_client_ip(&headers)?;

    Ok(Json(faucet.request_faucet(recipient, client_ip).await?))
}

fn extract_client_ip(headers: &HeaderMap) -> Result<String, AtlasError> {
    if let Some(value) = headers.get("x-forwarded-for") {
        let forwarded = value
            .to_str()
            .map_err(|_| AtlasError::InvalidInput("Invalid X-Forwarded-For header".to_string()))?;
        if let Some(ip) = forwarded
            .split(',')
            .next()
            .map(str::trim)
            .filter(|ip| !ip.is_empty())
        {
            return normalize_ip(ip);
        }
    }

    if let Some(value) = headers.get("x-real-ip") {
        let real_ip = value
            .to_str()
            .map_err(|_| AtlasError::InvalidInput("Invalid X-Real-IP header".to_string()))?;
        if !real_ip.trim().is_empty() {
            return normalize_ip(real_ip.trim());
        }
    }

    Err(AtlasError::InvalidInput(
        "Client IP is required for faucet requests".to_string(),
    ))
}

fn normalize_ip(ip: &str) -> Result<String, AtlasError> {
    let parsed = IpAddr::from_str(ip)
        .map_err(|_| AtlasError::InvalidInput("Invalid client IP address".to_string()))?;
    Ok(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::AppState;
    use crate::faucet::{FaucetBackend, FaucetInfo, FaucetTxResponse, SharedFaucetBackend};
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use futures::future::{BoxFuture, FutureExt};
    use tokio::sync::broadcast;
    use tower::util::ServiceExt;

    #[derive(Clone)]
    struct FakeFaucet;

    #[derive(Clone)]
    struct CoolingDownFaucet;

    impl FaucetBackend for FakeFaucet {
        fn info(&self) -> BoxFuture<'static, Result<FaucetInfo, atlas_common::AtlasError>> {
            async move {
                Ok(FaucetInfo {
                    amount_wei: "1000".to_string(),
                    balance_wei: "2000".to_string(),
                    cooldown_minutes: 30,
                })
            }
            .boxed()
        }

        fn request_faucet(
            &self,
            _recipient: Address,
            _client_ip: String,
        ) -> BoxFuture<'static, Result<FaucetTxResponse, atlas_common::AtlasError>> {
            async move {
                Ok(FaucetTxResponse {
                    tx_hash: "0xdeadbeef".to_string(),
                })
            }
            .boxed()
        }
    }

    impl FaucetBackend for CoolingDownFaucet {
        fn info(&self) -> BoxFuture<'static, Result<FaucetInfo, atlas_common::AtlasError>> {
            async move {
                Ok(FaucetInfo {
                    amount_wei: "1000".to_string(),
                    balance_wei: "2000".to_string(),
                    cooldown_minutes: 30,
                })
            }
            .boxed()
        }

        fn request_faucet(
            &self,
            _recipient: Address,
            _client_ip: String,
        ) -> BoxFuture<'static, Result<FaucetTxResponse, atlas_common::AtlasError>> {
            async move {
                Err(atlas_common::AtlasError::TooManyRequests {
                    message: "Faucet cooldown active".to_string(),
                    retry_after_seconds: 30,
                })
            }
            .boxed()
        }
    }

    fn test_state(faucet: Option<SharedFaucetBackend>) -> Arc<AppState> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost:5432/test")
            .expect("lazy pool");
        let head_tracker = Arc::new(crate::head::HeadTracker::empty(10));
        let (tx, _) = broadcast::channel(1);
        Arc::new(AppState {
            pool,
            block_events_tx: tx,
            head_tracker,
            rpc_url: String::new(),
            faucet,
        })
    }

    #[tokio::test]
    async fn faucet_info_route_is_available_when_enabled() {
        let faucet: SharedFaucetBackend = Arc::new(FakeFaucet);
        let app = Router::new()
            .route("/api/faucet/info", get(get_faucet_info))
            .with_state(test_state(Some(faucet)));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/faucet/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["amount_wei"], "1000");
        assert_eq!(value["balance_wei"], "2000");
        assert_eq!(value["cooldown_minutes"], 30);
    }

    #[tokio::test]
    async fn faucet_post_route_returns_tx_hash() {
        let faucet: SharedFaucetBackend = Arc::new(FakeFaucet);
        let app = Router::new()
            .route("/api/faucet", axum::routing::post(request_faucet))
            .with_state(test_state(Some(faucet)));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/faucet")
                    .header("content-type", "application/json")
                    .header("x-forwarded-for", "127.0.0.1")
                    .body(Body::from(
                        r#"{"address":"0x0000000000000000000000000000000000000001"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["tx_hash"], "0xdeadbeef");
    }

    #[tokio::test]
    async fn faucet_info_route_is_404_when_disabled() {
        let app = Router::new()
            .route("/api/faucet/info", get(get_faucet_info))
            .with_state(test_state(None));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/faucet/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn request_faucet_requires_client_ip() {
        let faucet: SharedFaucetBackend = Arc::new(FakeFaucet);
        let app = Router::new()
            .route("/api/faucet", axum::routing::post(request_faucet))
            .with_state(test_state(Some(faucet)));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/faucet")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"address":"0x0000000000000000000000000000000000000001"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn request_faucet_returns_retry_after_when_cooling_down() {
        let faucet: SharedFaucetBackend = Arc::new(CoolingDownFaucet);
        let app = Router::new()
            .route("/api/faucet", axum::routing::post(request_faucet))
            .with_state(test_state(Some(faucet)));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/faucet")
                    .header("content-type", "application/json")
                    .header("x-forwarded-for", "127.0.0.1")
                    .body(Body::from(
                        r#"{"address":"0x0000000000000000000000000000000000000001"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get("retry-after").unwrap(), "30");

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"], "Faucet cooldown active");
        assert_eq!(value["retry_after_seconds"], 30);
    }
}
