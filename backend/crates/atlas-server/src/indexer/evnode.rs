//! ev-node Connect RPC client for querying DA (Data Availability) status.
//!
//! ev-node exposes a Connect RPC service (`StoreService`) that provides
//! consensus/DA layer data separate from the standard EVM JSON-RPC API.
//! This module wraps the `GetBlock` RPC to extract DA inclusion heights.
//!
//! Uses the Connect RPC JSON codec (`application/json`), which ev-node
//! supports out of the box alongside protobuf.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Connect RPC JSON request for StoreService.GetBlock.
/// uint64 fields are encoded as strings per Connect RPC convention.
#[derive(Serialize)]
struct GetBlockRequest {
    height: String,
}

/// Connect RPC JSON response for StoreService.GetBlock.
/// We only extract the DA height fields.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetBlockResponse {
    #[serde(default, deserialize_with = "deserialize_u64_string")]
    header_da_height: u64,
    #[serde(default, deserialize_with = "deserialize_u64_string")]
    data_da_height: u64,
}

/// Connect RPC encodes uint64 as JSON strings (e.g., `"123"` not `123`).
/// This deserializer handles both string and numeric representations.
fn deserialize_u64_string<'de, D>(deserializer: D) -> std::result::Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct U64Visitor;
    impl<'de> de::Visitor<'de> for U64Visitor {
        type Value = u64;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a u64 as a string or number")
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<u64, E> {
            Ok(v)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<u64, E> {
            v.parse().map_err(de::Error::custom)
        }
    }
    deserializer.deserialize_any(U64Visitor)
}

/// Retry delays for ev-node RPC calls (in milliseconds).
/// Fail fast — the background loop will retry on the next cycle anyway.
const RETRY_DELAYS_MS: &[u64] = &[100, 500, 1000];
const MAX_RETRIES: usize = 3;

/// Client for ev-node's Connect RPC StoreService.
pub struct EvnodeClient {
    client: reqwest::Client,
    url: String,
}

impl EvnodeClient {
    /// Create a new client pointing at the given ev-node Connect RPC URL.
    pub fn new(evnode_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("failed to create HTTP client");

        let base = evnode_url.trim_end_matches('/');
        Self {
            client,
            url: format!("{base}/evnode.v1.StoreService/GetBlock"),
        }
    }

    /// Fetch DA inclusion heights for a block.
    ///
    /// Returns `(header_da_height, data_da_height)`.
    /// Both are 0 if the block has not yet been submitted to Celestia.
    ///
    /// Retries with backoff on transient errors.
    pub async fn get_da_status(&self, height: u64) -> Result<(u64, u64)> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match self.do_request(height).await {
                Ok((h, d)) => return Ok((h, d)),
                Err(e) => {
                    last_error = Some(e);
                    if attempt + 1 < MAX_RETRIES {
                        let delay_ms = RETRY_DELAYS_MS
                            .get(attempt)
                            .copied()
                            .unwrap_or(*RETRY_DELAYS_MS.last().unwrap());

                        tracing::warn!(
                            height,
                            attempt = attempt + 1,
                            max_retries = MAX_RETRIES,
                            error = %last_error.as_ref().unwrap(),
                            retry_in_ms = delay_ms,
                            "ev-node GetBlock failed"
                        );

                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        bail!(
            "ev-node GetBlock failed for height {} after {} attempts: {}",
            height,
            MAX_RETRIES,
            last_error.unwrap()
        )
    }

    async fn do_request(&self, height: u64) -> Result<(u64, u64)> {
        let request = GetBlockRequest {
            height: height.to_string(),
        };

        let response = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            bail!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }

        let resp: GetBlockResponse = response.json().await?;
        Ok((resp.header_da_height, resp.data_da_height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_trims_trailing_slash() {
        let client = EvnodeClient::new("http://localhost:7331/");
        assert_eq!(
            client.url,
            "http://localhost:7331/evnode.v1.StoreService/GetBlock"
        );
    }

    #[test]
    fn request_serializes_height_as_string() {
        let req = GetBlockRequest {
            height: 42.to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"height":"42"}"#);
    }

    #[test]
    fn response_deserializes_string_heights() {
        let json = r#"{"headerDaHeight":"100","dataDaHeight":"200"}"#;
        let resp: GetBlockResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.header_da_height, 100);
        assert_eq!(resp.data_da_height, 200);
    }

    #[test]
    fn response_deserializes_numeric_heights() {
        let json = r#"{"headerDaHeight":100,"dataDaHeight":200}"#;
        let resp: GetBlockResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.header_da_height, 100);
        assert_eq!(resp.data_da_height, 200);
    }

    #[test]
    fn response_defaults_missing_fields_to_zero() {
        let json = r#"{}"#;
        let resp: GetBlockResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.header_da_height, 0);
        assert_eq!(resp.data_da_height, 0);
    }
}
