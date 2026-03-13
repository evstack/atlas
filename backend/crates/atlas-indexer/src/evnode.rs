//! ev-node Connect RPC client for querying DA (Data Availability) status.
//!
//! ev-node exposes a Connect RPC service (`StoreService`) that provides
//! consensus/DA layer data separate from the standard EVM JSON-RPC API.
//! This module wraps the `GetBlock` RPC to extract DA inclusion heights.
//!
//! Connect RPC supports two serialization modes:
//! - **Protobuf** (`application/proto`) — binary, more efficient
//! - **JSON** (`application/json`) — text, required by some deployments
//!
//! The client auto-detects the correct mode: it starts with protobuf and
//! transparently switches to JSON if the server returns 415 Unsupported
//! Media Type. Once switched, all subsequent requests use JSON.

use anyhow::{bail, Result};
use prost::Message;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Protobuf message types (matching ev-node proto/evnode/v1/state_rpc.proto)
//
// We define only the minimal types needed to decode GetBlockResponse.
// The GetBlockResponse has top-level fields for DA heights (tags 2 and 3),
// so we don't need to navigate into nested Block/Header/Data messages.
// ---------------------------------------------------------------------------

/// Request message for StoreService.GetBlock.
/// Field 1: block height (uint64).
#[derive(Clone, PartialEq, Message)]
pub struct GetBlockRequest {
    #[prost(uint64, tag = "1")]
    pub height: u64,
}

/// Response message for StoreService.GetBlock (minimal).
/// We only decode the DA height fields, ignoring the full Block message.
#[derive(Clone, PartialEq, Message)]
pub struct GetBlockResponse {
    // Field 1 (Block) is skipped — we don't need block contents for DA status.

    /// Celestia height where the block header was submitted.
    /// 0 means not yet submitted.
    #[prost(uint64, tag = "2")]
    pub header_da_height: u64,

    /// Celestia height where the block data was submitted.
    /// 0 means not yet submitted.
    #[prost(uint64, tag = "3")]
    pub data_da_height: u64,
}

// ---------------------------------------------------------------------------
// JSON types for Connect RPC JSON mode
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsonGetBlockRequest {
    height: String, // Connect RPC encodes uint64 as string in JSON
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonGetBlockResponse {
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

/// Retry delays for ev-node RPC calls (in seconds).
const RETRY_DELAYS: &[u64] = &[2, 5, 10, 20, 30];
const MAX_RETRIES: usize = 10;

/// Client for ev-node's Connect RPC StoreService.
///
/// Supports both protobuf and JSON serialization modes. The mode is
/// auto-detected on the first request: if the server rejects protobuf
/// with HTTP 415, the client switches to JSON for all future requests.
pub struct EvnodeClient {
    client: reqwest::Client,
    base_url: String,
    /// When true, use JSON mode instead of protobuf.
    use_json: AtomicBool,
}

impl EvnodeClient {
    /// Create a new client pointing at the given ev-node Connect RPC URL.
    ///
    /// # Arguments
    /// * `evnode_url` — Base URL of the ev-node Connect RPC service (e.g., `http://localhost:7331`)
    pub fn new(evnode_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to create HTTP client");

        Self {
            client,
            base_url: evnode_url.trim_end_matches('/').to_string(),
            use_json: AtomicBool::new(false),
        }
    }

    /// Fetch DA inclusion heights for a block.
    ///
    /// Returns `(header_da_height, data_da_height)`.
    /// Both are 0 if the block has not yet been submitted to Celestia.
    ///
    /// Retries with exponential backoff on transient errors.
    pub async fn get_da_status(&self, height: u64) -> Result<(u64, u64)> {
        let url = format!(
            "{}/evnode.v1.StoreService/GetBlock",
            self.base_url
        );

        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match self.do_request(&url, height).await {
                Ok((h, d)) => return Ok((h, d)),
                Err(e) => {
                    let delay = RETRY_DELAYS
                        .get(attempt)
                        .copied()
                        .unwrap_or(*RETRY_DELAYS.last().unwrap());

                    tracing::warn!(
                        "ev-node GetBlock failed for height {} (attempt {}): {}. Retrying in {}s",
                        height,
                        attempt + 1,
                        e,
                        delay,
                    );

                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
            }
        }

        bail!(
            "ev-node GetBlock failed for height {} after {} retries: {}",
            height,
            MAX_RETRIES,
            last_error.unwrap()
        )
    }

    /// Send a Connect RPC request, auto-detecting proto vs JSON mode.
    ///
    /// On HTTP 415 (Unsupported Media Type) when using protobuf, switches
    /// to JSON mode and retries the request immediately.
    async fn do_request(&self, url: &str, height: u64) -> Result<(u64, u64)> {
        if self.use_json.load(Ordering::Relaxed) {
            return self.do_json_request(url, height).await;
        }

        // Try protobuf first
        let request = GetBlockRequest { height };
        let body = request.encode_to_vec();

        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/proto")
            .body(body)
            .send()
            .await?;

        // If server requires JSON, switch modes and retry
        if response.status() == reqwest::StatusCode::UNSUPPORTED_MEDIA_TYPE {
            tracing::info!("ev-node requires JSON mode, switching from protobuf");
            self.use_json.store(true, Ordering::Relaxed);
            return self.do_json_request(url, height).await;
        }

        if !response.status().is_success() {
            bail!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }

        let bytes = response.bytes().await?;
        let resp = GetBlockResponse::decode(bytes.as_ref())?;
        Ok((resp.header_da_height, resp.data_da_height))
    }

    /// Send a Connect RPC request using JSON serialization.
    async fn do_json_request(&self, url: &str, height: u64) -> Result<(u64, u64)> {
        let request = JsonGetBlockRequest {
            height: height.to_string(),
        };

        let response = self
            .client
            .post(url)
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

        let resp: JsonGetBlockResponse = response.json().await?;
        Ok((resp.header_da_height, resp.data_da_height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_get_block_request() {
        let req = GetBlockRequest { height: 42 };
        let bytes = req.encode_to_vec();
        let decoded = GetBlockRequest::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.height, 42);
    }

    #[test]
    fn encode_decode_get_block_response() {
        let resp = GetBlockResponse {
            header_da_height: 100,
            data_da_height: 200,
        };
        let bytes = resp.encode_to_vec();
        let decoded = GetBlockResponse::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.header_da_height, 100);
        assert_eq!(decoded.data_da_height, 200);
    }

    #[test]
    fn decode_response_with_zeros() {
        let resp = GetBlockResponse {
            header_da_height: 0,
            data_da_height: 0,
        };
        let bytes = resp.encode_to_vec();
        let decoded = GetBlockResponse::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.header_da_height, 0);
        assert_eq!(decoded.data_da_height, 0);
    }

    #[test]
    fn decode_empty_response_defaults_to_zeros() {
        // An empty protobuf message should decode with default (zero) values
        let decoded = GetBlockResponse::decode(&[] as &[u8]).unwrap();
        assert_eq!(decoded.header_da_height, 0);
        assert_eq!(decoded.data_da_height, 0);
    }

    #[test]
    fn client_trims_trailing_slash() {
        let client = EvnodeClient::new("http://localhost:7331/");
        assert_eq!(client.base_url, "http://localhost:7331");
    }

    #[test]
    fn client_starts_in_proto_mode() {
        let client = EvnodeClient::new("http://localhost:7331");
        assert!(!client.use_json.load(Ordering::Relaxed));
    }

    #[test]
    fn json_request_serializes_height_as_string() {
        let req = JsonGetBlockRequest {
            height: 42.to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"height":"42"}"#);
    }

    #[test]
    fn json_response_deserializes_string_heights() {
        let json = r#"{"headerDaHeight":"100","dataDaHeight":"200"}"#;
        let resp: JsonGetBlockResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.header_da_height, 100);
        assert_eq!(resp.data_da_height, 200);
    }

    #[test]
    fn json_response_deserializes_numeric_heights() {
        let json = r#"{"headerDaHeight":100,"dataDaHeight":200}"#;
        let resp: JsonGetBlockResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.header_da_height, 100);
        assert_eq!(resp.data_da_height, 200);
    }

    #[test]
    fn json_response_defaults_missing_fields_to_zero() {
        let json = r#"{}"#;
        let resp: JsonGetBlockResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.header_da_height, 0);
        assert_eq!(resp.data_da_height, 0);
    }
}
