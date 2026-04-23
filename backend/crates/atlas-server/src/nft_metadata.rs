use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use base64::Engine;
use chrono::{DateTime, Utc};
use percent_encoding::percent_decode_str;
use reqwest::StatusCode;

/// Maximum size of an NFT metadata payload (HTTP response body or data: URI).
const MAX_METADATA_BYTES: usize = 2 * 1024 * 1024; // 2 MB

pub const NFT_METADATA_PENDING: &str = "pending";
pub const NFT_METADATA_FETCHED: &str = "fetched";
pub const NFT_METADATA_RETRYABLE_ERROR: &str = "retryable_error";
pub const NFT_METADATA_PERMANENT_ERROR: &str = "permanent_error";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchErrorKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchError {
    pub kind: FetchErrorKind,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedMetadata {
    pub name: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FetchedMetadata {
    Json {
        metadata: serde_json::Value,
        extracted: ExtractedMetadata,
    },
    DirectImage {
        image_url: String,
    },
}

/// Custom DNS resolver that rejects non-public IP addresses.
///
/// Plugged into the reqwest client so that validation and the actual TCP
/// connection use the same resolved addresses, closing the DNS-rebinding
/// window that exists when they are done in two separate lookups.
pub struct SsrfSafeResolver;

impl reqwest::dns::Resolve for SsrfSafeResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            type DynErr = Box<dyn std::error::Error + Send + Sync>;

            let iter = tokio::net::lookup_host(format!("{}:0", host))
                .await
                .map_err(|e| Box::new(e) as DynErr)?;

            let addrs: Vec<SocketAddr> =
                iter.filter(|addr| !is_non_public_ip(&addr.ip())).collect();

            if addrs.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "non_public_metadata_host",
                )) as DynErr);
            }

            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

pub fn resolve_uri(uri: &str, ipfs_gateway: &str) -> String {
    if let Some(stripped) = uri.strip_prefix("ipfs://") {
        let stripped = stripped.strip_prefix("ipfs/").unwrap_or(stripped);
        format!("{}{}", ipfs_gateway, stripped)
    } else if let Some(stripped) = uri.strip_prefix("ar://") {
        format!("https://arweave.net/{}", stripped)
    } else {
        uri.to_string()
    }
}

pub fn extract_metadata_fields(
    metadata: &serde_json::Value,
    ipfs_gateway: &str,
) -> ExtractedMetadata {
    let name = metadata
        .get("name")
        .or_else(|| metadata.get("title"))
        .and_then(|value| value.as_str())
        .map(str::to_string);

    let image_url = metadata
        .get("image")
        .or_else(|| metadata.get("image_url"))
        .or_else(|| metadata.get("imageUrl"))
        .or_else(|| {
            metadata.get("image_data").filter(|value| {
                value
                    .as_str()
                    .is_some_and(|image| image.starts_with("data:image/"))
            })
        })
        .and_then(|value| value.as_str())
        .map(|image| resolve_uri(image, ipfs_gateway));

    ExtractedMetadata { name, image_url }
}

pub async fn fetch_metadata(
    client: &reqwest::Client,
    uri: &str,
    ipfs_gateway: &str,
) -> Result<FetchedMetadata, FetchError> {
    let url = resolve_uri(uri, ipfs_gateway);
    if url.starts_with("data:image/") {
        return Ok(FetchedMetadata::DirectImage { image_url: url });
    }
    if url.starts_with("data:") {
        let metadata = parse_data_json_uri(&url)?;
        let extracted = extract_metadata_fields(&metadata, ipfs_gateway);

        return Ok(FetchedMetadata::Json {
            metadata,
            extracted,
        });
    }

    validate_metadata_url_scheme(&url)?;
    // DNS rebinding protection is enforced by SsrfSafeResolver in the client.

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(classify_request_error)?;
    let status = response.status();
    if !status.is_success() {
        return Err(classify_status(status));
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");

    if content_type.starts_with("image/") {
        return Ok(FetchedMetadata::DirectImage { image_url: url });
    }

    if response
        .content_length()
        .is_some_and(|len| len as usize > MAX_METADATA_BYTES)
    {
        return Err(permanent_error("response_too_large"));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|_| retryable_error("response_read_error"))?;
    if bytes.len() > MAX_METADATA_BYTES {
        return Err(permanent_error("response_too_large"));
    }

    let metadata: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| permanent_error("json_parse_error"))?;
    let extracted = extract_metadata_fields(&metadata, ipfs_gateway);

    Ok(FetchedMetadata::Json {
        metadata,
        extracted,
    })
}

fn parse_data_json_uri(uri: &str) -> Result<serde_json::Value, FetchError> {
    let (header, payload) = uri
        .strip_prefix("data:")
        .and_then(|value| value.split_once(','))
        .ok_or_else(|| permanent_error("invalid_data_uri"))?;

    let mut parts = header.split(';');
    let media_type = parts.next().unwrap_or_default();
    let is_base64 = parts.any(|part| part.eq_ignore_ascii_case("base64"));

    if !is_json_media_type(media_type) {
        return Err(permanent_error("unsupported_data_uri_media_type"));
    }

    if payload.len() > MAX_METADATA_BYTES {
        return Err(permanent_error("data_uri_too_large"));
    }

    let bytes = if is_base64 {
        base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|_| permanent_error("invalid_data_uri_base64"))?
    } else {
        percent_decode_str(payload).collect::<Vec<u8>>()
    };

    serde_json::from_slice(&bytes).map_err(|_| permanent_error("json_parse_error"))
}

fn is_json_media_type(media_type: &str) -> bool {
    matches!(media_type, "application/json" | "text/json") || media_type.ends_with("+json")
}

pub fn schedule_retry(
    retry_count: i32,
    max_retry_attempts: u32,
    now: DateTime<Utc>,
) -> RetryDecision {
    if retry_count as u32 > max_retry_attempts {
        return RetryDecision::PermanentError;
    }

    let delay = match retry_count {
        1 => Duration::from_secs(5 * 60),
        2 => Duration::from_secs(30 * 60),
        3 => Duration::from_secs(6 * 60 * 60),
        _ => Duration::from_secs(24 * 60 * 60),
    };

    RetryDecision::RetryAt(now + chrono::Duration::from_std(delay).expect("retry delay"))
}

#[derive(Debug, Clone, PartialEq)]
pub enum RetryDecision {
    RetryAt(DateTime<Utc>),
    PermanentError,
}

fn classify_status(status: StatusCode) -> FetchError {
    match status {
        StatusCode::BAD_REQUEST => permanent_error("http_400"),
        StatusCode::UNAUTHORIZED => permanent_error("http_401"),
        StatusCode::FORBIDDEN => permanent_error("http_403"),
        StatusCode::NOT_FOUND => permanent_error("http_404"),
        StatusCode::REQUEST_TIMEOUT => retryable_error("http_408"),
        StatusCode::GONE => permanent_error("http_410"),
        StatusCode::UNSUPPORTED_MEDIA_TYPE => permanent_error("http_415"),
        StatusCode::TOO_MANY_REQUESTS => retryable_error("http_429"),
        StatusCode::INTERNAL_SERVER_ERROR => retryable_error("http_500"),
        StatusCode::BAD_GATEWAY => retryable_error("http_502"),
        StatusCode::SERVICE_UNAVAILABLE => retryable_error("http_503"),
        StatusCode::GATEWAY_TIMEOUT => retryable_error("http_504"),
        _ if status.is_server_error() => retryable_error(format!("http_{}", status.as_u16())),
        _ => permanent_error(format!("http_{}", status.as_u16())),
    }
}

fn classify_request_error(error: reqwest::Error) -> FetchError {
    if error.is_timeout() {
        return retryable_error("request_timeout");
    }
    if error.is_connect() {
        return retryable_error("request_connect_error");
    }
    if error.is_request() {
        return permanent_error("invalid_metadata_url");
    }

    retryable_error("request_error")
}

fn validate_metadata_url_scheme(url: &str) -> Result<(), FetchError> {
    let parsed = reqwest::Url::parse(url).map_err(|_| permanent_error("invalid_metadata_url"))?;

    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err(permanent_error("disallowed_url_scheme")),
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| permanent_error("missing_metadata_host"))?;

    // reqwest 0.13 bypasses SsrfSafeResolver for IP-literal hosts; block them explicitly.
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_non_public_ip(&ip) {
            return Err(permanent_error("non_public_metadata_host"));
        }
    }

    Ok(())
}

fn is_non_public_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_non_public_ip(&IpAddr::V4(v4));
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xFFC0) == 0xFE80
                || (v6.segments()[0] & 0xFE00) == 0xFC00
        }
    }
}

fn retryable_error(code: impl Into<String>) -> FetchError {
    FetchError {
        kind: FetchErrorKind::Retryable,
        code: code.into(),
    }
}

fn permanent_error(code: impl Into<String>) -> FetchError {
    FetchError {
        kind: FetchErrorKind::Permanent,
        code: code.into(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn extracts_image_aliases_from_raw_metadata() {
        let metadata = serde_json::json!({
            "title": "Fallback Name",
            "image_url": "ipfs://ipfs/QmImage123"
        });

        let extracted = extract_metadata_fields(&metadata, "https://ipfs.io/ipfs/");

        assert_eq!(extracted.name.as_deref(), Some("Fallback Name"));
        assert_eq!(
            extracted.image_url.as_deref(),
            Some("https://ipfs.io/ipfs/QmImage123")
        );
    }

    #[test]
    fn capped_retries_become_permanent_errors() {
        let now = Utc.with_ymd_and_hms(2026, 4, 22, 18, 0, 0).unwrap();

        assert_eq!(
            schedule_retry(1, 3, now),
            RetryDecision::RetryAt(Utc.with_ymd_and_hms(2026, 4, 22, 18, 5, 0).unwrap())
        );
        assert_eq!(schedule_retry(4, 3, now), RetryDecision::PermanentError);
    }

    #[tokio::test]
    async fn parses_base64_json_data_uri_metadata() {
        let client = reqwest::Client::new();
        let payload = base64::engine::general_purpose::STANDARD
            .encode(r#"{"name":"Onchain NFT","image":"ipfs://QmImage123"}"#);

        let fetched = fetch_metadata(
            &client,
            &format!("data:application/json;base64,{payload}"),
            "https://ipfs.io/ipfs/",
        )
        .await
        .expect("fetch metadata from data uri");

        match fetched {
            FetchedMetadata::Json {
                metadata,
                extracted,
            } => {
                assert_eq!(metadata["name"], "Onchain NFT");
                assert_eq!(extracted.name.as_deref(), Some("Onchain NFT"));
                assert_eq!(
                    extracted.image_url.as_deref(),
                    Some("https://ipfs.io/ipfs/QmImage123")
                );
            }
            other => panic!("expected json metadata, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn parses_percent_encoded_json_data_uri_metadata() {
        let client = reqwest::Client::new();
        let fetched = fetch_metadata(
            &client,
            "data:application/json,%7B%22description%22%3A%22Onchain%20metadata%22%7D",
            "https://ipfs.io/ipfs/",
        )
        .await
        .expect("fetch metadata from percent-encoded data uri");

        match fetched {
            FetchedMetadata::Json { metadata, .. } => {
                assert_eq!(metadata["description"], "Onchain metadata");
            }
            other => panic!("expected json metadata, got {other:?}"),
        }
    }
}
