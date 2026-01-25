//! Contract verification handlers
//!
//! Provides endpoints for verifying smart contracts by compiling source code
//! and matching against deployed bytecode.

use alloy::providers::{Provider, ProviderBuilder};
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;

use atlas_common::{
    AtlasError, ContractAbiResponse, ContractSourceResponse, VerifiedContract,
    VerifyContractRequest, VerifyContractResponse,
};
use crate::AppState;
use crate::error::ApiResult;

/// Solc compiler output structure
#[derive(Debug, Deserialize)]
struct SolcOutput {
    contracts: Option<std::collections::HashMap<String, std::collections::HashMap<String, SolcContract>>>,
    errors: Option<Vec<SolcError>>,
}

#[derive(Debug, Deserialize)]
struct SolcContract {
    abi: Option<serde_json::Value>,
    evm: Option<SolcEvm>,
}

#[derive(Debug, Deserialize)]
struct SolcEvm {
    bytecode: Option<SolcBytecode>,
    #[serde(rename = "deployedBytecode")]
    deployed_bytecode: Option<SolcBytecode>,
}

#[derive(Debug, Deserialize)]
struct SolcBytecode {
    object: String,
}

#[derive(Debug, Deserialize)]
struct SolcError {
    severity: String,
    message: String,
    #[serde(rename = "formattedMessage")]
    formatted_message: Option<String>,
}

/// Standard JSON input format for solc
#[derive(Debug, Serialize)]
struct SolcStandardInput {
    language: String,
    sources: std::collections::HashMap<String, SolcSource>,
    settings: SolcSettings,
}

#[derive(Debug, Serialize)]
struct SolcSource {
    content: String,
}

#[derive(Debug, Serialize)]
struct SolcSettings {
    optimizer: SolcOptimizer,
    #[serde(rename = "evmVersion", skip_serializing_if = "Option::is_none")]
    evm_version: Option<String>,
    #[serde(rename = "outputSelection")]
    output_selection: std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>>,
}

#[derive(Debug, Serialize)]
struct SolcOptimizer {
    enabled: bool,
    runs: u32,
}

/// POST /api/contracts/verify - Verify contract source code
pub async fn verify_contract(
    State(state): State<Arc<AppState>>,
    Json(request): Json<VerifyContractRequest>,
) -> ApiResult<Json<VerifyContractResponse>> {
    let address = normalize_address(&request.address);

    // Validate address format
    if !is_valid_address(&address) {
        return Err(AtlasError::InvalidInput("Invalid contract address".to_string()).into());
    }

    // Check if already verified
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT address FROM contract_abis WHERE LOWER(address) = LOWER($1)"
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    if existing.is_some() {
        return Ok(Json(VerifyContractResponse {
            success: false,
            address: address.clone(),
            message: Some("Contract is already verified".to_string()),
            abi: None,
        }));
    }

    // Fetch deployed bytecode from RPC
    let deployed_bytecode = fetch_deployed_bytecode(&state.rpc_url, &address).await?;

    if deployed_bytecode.is_empty() || deployed_bytecode == "0x" {
        return Err(AtlasError::Verification(
            "No bytecode found at address. Is this a contract?".to_string()
        ).into());
    }

    // Compile the source code
    let (abi, compiled_bytecode) = compile_contract(&request, &state.solc_path).await?;

    // Compare bytecodes (strip metadata hash)
    let deployed_stripped = strip_metadata_hash(&deployed_bytecode);
    let compiled_stripped = strip_metadata_hash(&compiled_bytecode);

    if !bytecodes_match(&deployed_stripped, &compiled_stripped, &request.constructor_args) {
        return Err(AtlasError::BytecodeMismatch(
            "Compiled bytecode does not match deployed bytecode. Check compiler settings.".to_string()
        ).into());
    }

    // Parse constructor args
    let constructor_args_bytes = request.constructor_args.as_ref()
        .map(|args| hex::decode(args.trim_start_matches("0x")))
        .transpose()
        .map_err(|_| AtlasError::InvalidInput("Invalid constructor arguments hex".to_string()))?;

    // Determine if multi-file
    let is_multi_file = request.is_standard_json;
    let source_files: Option<serde_json::Value> = if is_multi_file {
        // Parse the standard JSON to extract source files
        serde_json::from_str(&request.source_code).ok()
            .and_then(|v: serde_json::Value| v.get("sources").cloned())
    } else {
        None
    };

    // Store verified contract
    sqlx::query(
        r#"
        INSERT INTO contract_abis (
            address, abi, source_code, compiler_version, optimization_used, runs,
            contract_name, constructor_args, evm_version, license_type, is_multi_file, source_files, verified_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW())
        "#
    )
    .bind(&address)
    .bind(&abi)
    .bind(&request.source_code)
    .bind(&request.compiler_version)
    .bind(request.optimization_enabled)
    .bind(request.optimization_runs as i32)
    .bind(&request.contract_name)
    .bind(&constructor_args_bytes)
    .bind(&request.evm_version)
    .bind(&request.license_type)
    .bind(is_multi_file)
    .bind(&source_files)
    .execute(&state.pool)
    .await?;

    tracing::info!(address = %address, "Contract verified successfully");

    Ok(Json(VerifyContractResponse {
        success: true,
        address,
        message: Some("Contract verified successfully".to_string()),
        abi: Some(abi),
    }))
}

/// GET /api/contracts/:address/abi - Get verified contract ABI
pub async fn get_contract_abi(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<ContractAbiResponse>> {
    let address = normalize_address(&address);

    let contract: Option<(String, serde_json::Value, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT address, abi, contract_name, verified_at FROM contract_abis WHERE LOWER(address) = LOWER($1)"
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    match contract {
        Some((addr, abi, contract_name, verified_at)) => {
            Ok(Json(ContractAbiResponse {
                address: addr,
                abi,
                contract_name,
                verified_at,
            }))
        }
        None => Err(AtlasError::NotFound(format!(
            "No verified contract found at address {}",
            address
        )).into()),
    }
}

/// GET /api/contracts/:address/source - Get verified contract source code
pub async fn get_contract_source(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<ContractSourceResponse>> {
    let address = normalize_address(&address);

    let contract: Option<VerifiedContract> = sqlx::query_as(
        r#"
        SELECT address, abi, source_code, compiler_version, optimization_used, runs, verified_at,
               contract_name, constructor_args, evm_version, license_type, is_multi_file, source_files
        FROM contract_abis
        WHERE LOWER(address) = LOWER($1)
        "#
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    match contract {
        Some(c) => {
            let constructor_args_hex = c.constructor_args
                .as_ref()
                .map(|bytes| format!("0x{}", hex::encode(bytes)));

            Ok(Json(ContractSourceResponse {
                address: c.address,
                contract_name: c.contract_name,
                source_code: c.source_code.unwrap_or_default(),
                compiler_version: c.compiler_version,
                optimization_enabled: c.optimization_used.unwrap_or(false),
                optimization_runs: c.runs.unwrap_or(200),
                evm_version: c.evm_version,
                license_type: c.license_type,
                constructor_args: constructor_args_hex,
                is_multi_file: c.is_multi_file,
                source_files: c.source_files,
                verified_at: c.verified_at,
            }))
        }
        None => Err(AtlasError::NotFound(format!(
            "No verified contract found at address {}",
            address
        )).into()),
    }
}

/// Fetch deployed bytecode from the RPC endpoint
async fn fetch_deployed_bytecode(rpc_url: &str, address: &str) -> Result<String, AtlasError> {
    let provider = ProviderBuilder::new()
        .on_http(rpc_url.parse().map_err(|e| AtlasError::Config(format!("Invalid RPC URL: {}", e)))?);

    let addr: alloy::primitives::Address = address.parse()
        .map_err(|_| AtlasError::InvalidInput("Invalid address format".to_string()))?;

    let code = provider.get_code_at(addr).await
        .map_err(|e| AtlasError::Rpc(format!("Failed to fetch bytecode: {}", e)))?;

    Ok(format!("0x{}", hex::encode(code.as_ref())))
}

/// Compile contract using solc
async fn compile_contract(
    request: &VerifyContractRequest,
    solc_path: &str,
) -> Result<(serde_json::Value, String), AtlasError> {
    // Build standard JSON input
    let input = if request.is_standard_json {
        // Use provided standard JSON directly, but ensure output selection is correct
        let mut parsed: serde_json::Value = serde_json::from_str(&request.source_code)
            .map_err(|e| AtlasError::InvalidInput(format!("Invalid standard JSON input: {}", e)))?;

        // Ensure we have the required output selections
        if let Some(settings) = parsed.get_mut("settings") {
            if let Some(obj) = settings.as_object_mut() {
                let output_selection = serde_json::json!({
                    "*": {
                        "*": ["abi", "evm.bytecode.object", "evm.deployedBytecode.object"]
                    }
                });
                obj.insert("outputSelection".to_string(), output_selection);
            }
        }

        serde_json::to_string(&parsed)
            .map_err(|e| AtlasError::Internal(format!("Failed to serialize JSON: {}", e)))?
    } else {
        // Build standard JSON input from single file
        let mut sources = std::collections::HashMap::new();
        sources.insert(
            format!("{}.sol", request.contract_name.split(':').next().unwrap_or(&request.contract_name)),
            SolcSource {
                content: request.source_code.clone(),
            },
        );

        let mut output_selection = std::collections::HashMap::new();
        let mut file_selection = std::collections::HashMap::new();
        file_selection.insert(
            "*".to_string(),
            vec![
                "abi".to_string(),
                "evm.bytecode.object".to_string(),
                "evm.deployedBytecode.object".to_string(),
            ],
        );
        output_selection.insert("*".to_string(), file_selection);

        let input = SolcStandardInput {
            language: "Solidity".to_string(),
            sources,
            settings: SolcSettings {
                optimizer: SolcOptimizer {
                    enabled: request.optimization_enabled,
                    runs: request.optimization_runs,
                },
                evm_version: request.evm_version.clone(),
                output_selection,
            },
        };

        serde_json::to_string(&input)
            .map_err(|e| AtlasError::Internal(format!("Failed to serialize input: {}", e)))?
    };

    // Determine solc binary path - support version-specific binaries
    let version_clean = request.compiler_version
        .trim_start_matches('v')
        .split('+')
        .next()
        .unwrap_or(&request.compiler_version);

    // Try version-specific binary first, then fall back to configured path
    let solc_binary = format!("{}-{}", solc_path, version_clean);
    let solc_to_use = if tokio::fs::metadata(&solc_binary).await.is_ok() {
        solc_binary
    } else {
        solc_path.to_string()
    };

    // Run solc
    let mut cmd = Command::new(&solc_to_use);
    cmd.arg("--standard-json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()
        .map_err(|e| AtlasError::Compilation(format!("Failed to spawn solc: {}. Is solc installed at '{}'?", e, solc_to_use)))?;

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(input.as_bytes()).await
            .map_err(|e| AtlasError::Compilation(format!("Failed to write to solc stdin: {}", e)))?;
    }

    let output = child.wait_with_output().await
        .map_err(|e| AtlasError::Compilation(format!("Failed to wait for solc: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AtlasError::Compilation(format!("solc failed: {}", stderr)));
    }

    // Parse output
    let solc_output: SolcOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| AtlasError::Compilation(format!("Failed to parse solc output: {}", e)))?;

    // Check for errors
    if let Some(errors) = &solc_output.errors {
        let error_msgs: Vec<_> = errors.iter()
            .filter(|e| e.severity == "error")
            .map(|e| e.formatted_message.as_ref().unwrap_or(&e.message).clone())
            .collect();

        if !error_msgs.is_empty() {
            return Err(AtlasError::Compilation(error_msgs.join("\n")));
        }
    }

    // Find the contract
    let contracts = solc_output.contracts
        .ok_or_else(|| AtlasError::Compilation("No contracts in output".to_string()))?;

    // Parse contract name - could be "ContractName" or "path/File.sol:ContractName"
    let contract_name = request.contract_name
        .split(':')
        .next_back()
        .unwrap_or(&request.contract_name);

    // Find the contract in any file
    let mut found_contract: Option<&SolcContract> = None;
    let mut found_file: Option<&str> = None;

    for (file, file_contracts) in &contracts {
        if let Some(contract) = file_contracts.get(contract_name) {
            found_contract = Some(contract);
            found_file = Some(file);
            break;
        }
    }

    let contract = found_contract
        .ok_or_else(|| AtlasError::Compilation(format!(
            "Contract '{}' not found in compilation output. Available contracts: {:?}",
            contract_name,
            contracts.iter()
                .flat_map(|(_, c)| c.keys())
                .collect::<Vec<_>>()
        )))?;

    let abi = contract.abi.clone()
        .ok_or_else(|| AtlasError::Compilation("No ABI in contract output".to_string()))?;

    let evm = contract.evm.as_ref()
        .ok_or_else(|| AtlasError::Compilation("No EVM output in contract".to_string()))?;

    let deployed_bytecode = evm.deployed_bytecode.as_ref()
        .ok_or_else(|| AtlasError::Compilation("No deployed bytecode in output".to_string()))?;

    let bytecode = format!("0x{}", deployed_bytecode.object);

    tracing::debug!(
        contract_name = %contract_name,
        file = ?found_file,
        bytecode_len = bytecode.len(),
        "Compiled contract"
    );

    Ok((abi, bytecode))
}

/// Strip metadata hash from bytecode for comparison
/// Solidity appends CBOR-encoded metadata at the end of bytecode
fn strip_metadata_hash(bytecode: &str) -> String {
    let bytecode = bytecode.trim_start_matches("0x");

    // Metadata is typically the last 43-53 bytes depending on version
    // Format: a2 64 'ipfs' 58 22 <32-byte-hash> 64 'solc' 43 <version-bytes>
    // We'll look for the 'a2 64' or 'a1 65' CBOR markers

    if bytecode.len() < 100 {
        return bytecode.to_string();
    }

    // Try to find the CBOR metadata marker
    // Common patterns: "a264" (2-item map with 4-char key) or "a265" (2-item map with 5-char key)
    let patterns = ["a264697066735822", "a265627a7a7232"]; // "ipfs" and "bzzr2" markers

    for pattern in patterns {
        if let Some(pos) = bytecode.rfind(pattern) {
            // Verify this looks like metadata by checking position
            // Metadata should be at the end, within ~100 bytes
            if bytecode.len() - pos < 200 {
                return bytecode[..pos].to_string();
            }
        }
    }

    // Fallback: strip last 86 characters (43 bytes = typical metadata length)
    if bytecode.len() > 86 {
        bytecode[..bytecode.len() - 86].to_string()
    } else {
        bytecode.to_string()
    }
}

/// Compare bytecodes, accounting for constructor args
fn bytecodes_match(deployed: &str, compiled: &str, _constructor_args: &Option<String>) -> bool {
    let deployed = deployed.to_lowercase();
    let compiled = compiled.to_lowercase();

    // Direct match
    if deployed == compiled {
        return true;
    }

    // If constructor args provided, the deployed code might have them appended
    // to the creation bytecode, but runtime bytecode should match
    // Since we're comparing deployed (runtime) bytecode, they should match directly

    // Allow for minor differences in metadata stripping
    let min_len = deployed.len().min(compiled.len());

    // If one is significantly shorter, likely an issue
    if min_len < deployed.len().max(compiled.len()) * 90 / 100 {
        return false;
    }

    // Compare the common prefix
    deployed[..min_len] == compiled[..min_len]
}

fn normalize_address(address: &str) -> String {
    let addr = address.trim().to_lowercase();
    if addr.starts_with("0x") {
        addr
    } else {
        format!("0x{}", addr)
    }
}

fn is_valid_address(address: &str) -> bool {
    let addr = address.trim_start_matches("0x");
    addr.len() == 40 && addr.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_metadata_hash() {
        // Bytecode with IPFS metadata
        let bytecode = "608060405234801561001057600080fd5ba264697066735822122012345678901234567890123456789012345678901234567890123456789012345678901234";
        let stripped = strip_metadata_hash(bytecode);
        assert!(stripped.len() < bytecode.len());
        assert!(!stripped.contains("a264697066735822"));
    }

    #[test]
    fn test_normalize_address() {
        assert_eq!(normalize_address("0xABC123"), "0xabc123");
        assert_eq!(normalize_address("ABC123"), "0xabc123");
        assert_eq!(normalize_address("  0xDEF456  "), "0xdef456");
    }

    #[test]
    fn test_is_valid_address() {
        assert!(is_valid_address("0x1234567890123456789012345678901234567890"));
        assert!(is_valid_address("1234567890123456789012345678901234567890"));
        assert!(!is_valid_address("0x12345")); // too short
        assert!(!is_valid_address("0xGGGG567890123456789012345678901234567890")); // invalid hex
    }

    #[test]
    fn test_bytecodes_match() {
        assert!(bytecodes_match("abcd1234", "abcd1234", &None));
        assert!(bytecodes_match("ABCD1234", "abcd1234", &None)); // case insensitive
        assert!(!bytecodes_match("abcd1234", "efgh5678", &None));
    }
}
