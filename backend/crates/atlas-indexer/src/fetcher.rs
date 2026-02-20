use alloy::network::Ethereum;
use alloy::providers::{Provider, RootProvider};
use alloy::rpc::types::{Block, TransactionReceipt};
use alloy::transports::http::{Client, Http};
use anyhow::Result;
use governor::RateLimiter;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

/// Retry delays for RPC calls (in seconds)
const RPC_RETRY_DELAYS: &[u64] = &[2, 5, 10, 20, 30];
const RPC_MAX_RETRIES: usize = 10;

/// Work item for a worker - a range of blocks to fetch
#[derive(Debug, Clone)]
pub(crate) struct WorkItem {
    pub(crate) start_block: u64,
    pub(crate) count: usize,
}

pub(crate) type HttpProvider = RootProvider<Http<Client>, Ethereum>;
pub(crate) type SharedRateLimiter = Arc<RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>;

/// Result of fetching a block from RPC
pub(crate) enum FetchResult {
    Success(FetchedBlock),
    Error { block_num: u64, error: String },
}

/// Data fetched from RPC for a single block
pub(crate) struct FetchedBlock {
    pub(crate) number: u64,
    pub(crate) block: Block,
    pub(crate) receipts: Vec<TransactionReceipt>,
}

pub(crate) async fn fetch_blocks_batch(
    client: &reqwest::Client,
    rpc_url: &str,
    start_block: u64,
    count: usize,
    rate_limiter: &SharedRateLimiter,
) -> Vec<FetchResult> {
    tracing::debug!("Fetching batch: blocks {} to {}", start_block, start_block + count as u64 - 1);

    // Wait for rate limiter - we're making 2*count RPC calls in one HTTP request
    for _ in 0..(count * 2) {
        rate_limiter.until_ready().await;
    }

    // Build batch request: eth_getBlockByNumber + eth_getBlockReceipts per block
    let mut batch_request = Vec::with_capacity(count * 2);
    for i in 0..count {
        let block_num = start_block + i as u64;
        let block_hex = format!("0x{:x}", block_num);

        // eth_getBlockByNumber with full transactions
        batch_request.push(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": [block_hex, true],
            "id": i * 2
        }));

        // eth_getBlockReceipts
        batch_request.push(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockReceipts",
            "params": [block_hex],
            "id": i * 2 + 1
        }));
    }

    // Send batch request with retry for network errors
    let mut batch_response: Option<Vec<serde_json::Value>> = None;
    let mut last_error: Option<String> = None;

    for attempt in 0..RPC_MAX_RETRIES {
        // Send request
        let response = match client
            .post(rpc_url)
            .json(&batch_request)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let delay = RPC_RETRY_DELAYS
                    .get(attempt)
                    .copied()
                    .unwrap_or(*RPC_RETRY_DELAYS.last().unwrap_or(&30));

                tracing::warn!(
                    "RPC batch request failed (attempt {}/{}): {}. Retrying in {}s...",
                    attempt + 1,
                    RPC_MAX_RETRIES,
                    e,
                    delay
                );

                last_error = Some(format!("HTTP request failed: {}", e));
                tokio::time::sleep(Duration::from_secs(delay)).await;
                continue;
            }
        };

        // Parse response
        match response.json::<Vec<serde_json::Value>>().await {
            Ok(resp) => {
                if attempt > 0 {
                    tracing::info!(
                        "RPC batch request succeeded after {} retries (blocks {} to {})",
                        attempt,
                        start_block,
                        start_block + count as u64 - 1
                    );
                }
                batch_response = Some(resp);
                break;
            }
            Err(e) => {
                let delay = RPC_RETRY_DELAYS
                    .get(attempt)
                    .copied()
                    .unwrap_or(*RPC_RETRY_DELAYS.last().unwrap_or(&30));

                tracing::warn!(
                    "Failed to parse RPC response (attempt {}/{}): {}. Retrying in {}s...",
                    attempt + 1,
                    RPC_MAX_RETRIES,
                    e,
                    delay
                );

                last_error = Some(format!("Failed to parse response: {}", e));
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }
        }
    }

    // If all retries failed, return errors for all blocks
    let batch_response = match batch_response {
        Some(resp) => resp,
        None => {
            let error_msg = last_error.unwrap_or_else(|| "Unknown error".to_string());
            return (0..count)
                .map(|i| FetchResult::Error {
                    block_num: start_block + i as u64,
                    error: error_msg.clone(),
                })
                .collect();
        }
    };

    // Process responses - they should be in order by ID
    let mut results = Vec::with_capacity(count);
    let mut response_map: BTreeMap<u64, &serde_json::Value> = BTreeMap::new();

    for resp in &batch_response {
        if let Some(id) = resp.get("id").and_then(|v| v.as_u64()) {
            response_map.insert(id, resp);
        }
    }

    for i in 0..count {
        let block_num = start_block + i as u64;
        let block_id = (i * 2) as u64;
        let receipts_id = (i * 2 + 1) as u64;

        // Get block response
        let block_result = match response_map.get(&block_id) {
            Some(resp) => {
                if let Some(error) = resp.get("error") {
                    Err(format!("RPC error: {}", error))
                } else if let Some(result) = resp.get("result") {
                    if result.is_null() {
                        Err(format!("Block {} not found", block_num))
                    } else {
                        serde_json::from_value::<Block>(result.clone())
                            .map_err(|e| format!("Failed to parse block: {}", e))
                    }
                } else {
                    Err("No result in response".to_string())
                }
            }
            None => Err(format!("Missing response for block {}", block_num)),
        };

        // Get receipts response
        let receipts_result = match response_map.get(&receipts_id) {
            Some(resp) => {
                if let Some(error) = resp.get("error") {
                    Err(format!("RPC error: {}", error))
                } else if let Some(result) = resp.get("result") {
                    if result.is_null() {
                        Ok(Vec::new())
                    } else {
                        serde_json::from_value::<Vec<TransactionReceipt>>(result.clone())
                            .map_err(|e| format!("Failed to parse receipts: {}", e))
                    }
                } else {
                    Ok(Vec::new())
                }
            }
            None => Ok(Vec::new()),
        };

        // Combine block + receipts into a single result
        match (block_result, receipts_result) {
            (Ok(block), Ok(receipts)) => {
                tracing::debug!("Block {} complete ({} receipts)", block_num, receipts.len());
                results.push(FetchResult::Success(FetchedBlock {
                    number: block_num,
                    block,
                    receipts,
                }));
            }
            (Err(e), _) => {
                tracing::warn!("Failed to fetch block {}: {}", block_num, e);
                results.push(FetchResult::Error {
                    block_num,
                    error: e,
                });
            }
            (_, Err(e)) => {
                tracing::warn!("Failed to fetch receipts for block {}: {}", block_num, e);
                results.push(FetchResult::Error {
                    block_num,
                    error: e,
                });
            }
        }
    }

    results
}

/// Get block number with internal retry logic for network failures
pub(crate) async fn get_block_number_with_retry(provider: &HttpProvider) -> Result<u64> {
    let mut last_error = None;

    for attempt in 0..RPC_MAX_RETRIES {
        match provider.get_block_number().await {
            Ok(block_num) => {
                if attempt > 0 {
                    tracing::info!("RPC connection restored after {} retries", attempt);
                }
                return Ok(block_num);
            }
            Err(e) => {
                let delay = RPC_RETRY_DELAYS
                    .get(attempt)
                    .copied()
                    .unwrap_or(*RPC_RETRY_DELAYS.last().unwrap_or(&30));

                tracing::warn!(
                    "RPC request failed (attempt {}/{}): {}. Retrying in {}s...",
                    attempt + 1,
                    RPC_MAX_RETRIES,
                    e,
                    delay
                );

                last_error = Some(e);
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }
        }
    }

    Err(anyhow::anyhow!(
        "RPC connection failed after {} retries: {:?}",
        RPC_MAX_RETRIES,
        last_error
    ))
}
