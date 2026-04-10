//! Contract verification API
//!
//! POST /api/contracts/:address/verify — compile submitted Solidity source and validate
//! against on-chain bytecode. On success, stores ABI + source in `contract_abis`.
//!
//! GET /api/contracts/:address — returns verification status, ABI, and source.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env::consts::{ARCH, OS};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::api::error::ApiResult;
use crate::api::AppState;
use atlas_common::{AtlasError, FullContractAbi};

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    /// Single-file Solidity source (mutually exclusive with `source_files`)
    pub source_code: Option<String>,
    /// Multi-file source map: filename → content (mutually exclusive with `source_code`)
    pub source_files: Option<HashMap<String, String>>,
    /// Exact compiler version, e.g. "v0.8.20+commit.a1b79de6"
    pub compiler_version: String,
    pub optimization_enabled: bool,
    /// Optimizer runs (default 200)
    pub optimization_runs: Option<i32>,
    pub contract_name: String,
    /// Hex-encoded constructor arguments (without 0x prefix), optional
    pub constructor_args: Option<String>,
    /// EVM version, e.g. "paris" (default: compiler default)
    pub evm_version: Option<String>,
    pub license_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    pub verified: bool,
    pub abi: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImmutableReference {
    start: usize,
    length: usize,
}

#[derive(Debug)]
struct CompiledContract {
    bytecode: Vec<u8>,
    abi: serde_json::Value,
    immutable_references: Vec<ImmutableReference>,
}

#[derive(Debug, Serialize)]
pub struct ContractDetailResponse {
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abi: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimization_used: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runs: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evm_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_type: Option<String>,
    pub is_multi_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_files: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /api/contracts/:address
pub async fn get_contract(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> ApiResult<Json<ContractDetailResponse>> {
    let address = normalize_address(&address);

    let row: Option<FullContractAbi> = sqlx::query_as(
        "SELECT address, abi, source_code, compiler_version, optimization_used, runs,
                verified_at, contract_name, constructor_args, evm_version, license_type,
                is_multi_file, source_files
         FROM contract_abis
         WHERE address = $1",
    )
    .bind(&address)
    .fetch_optional(&state.pool)
    .await?;

    match row {
        None => Ok(Json(ContractDetailResponse {
            verified: false,
            address: None,
            abi: None,
            source_code: None,
            compiler_version: None,
            optimization_used: None,
            runs: None,
            contract_name: None,
            evm_version: None,
            license_type: None,
            is_multi_file: false,
            source_files: None,
            verified_at: None,
        })),
        Some(c) => Ok(Json(ContractDetailResponse {
            verified: true,
            address: Some(c.address),
            abi: Some(c.abi),
            source_code: c.source_code,
            compiler_version: c.compiler_version,
            optimization_used: c.optimization_used,
            runs: c.runs,
            contract_name: c.contract_name,
            evm_version: c.evm_version,
            license_type: c.license_type,
            is_multi_file: c.is_multi_file,
            source_files: c.source_files,
            verified_at: Some(c.verified_at),
        })),
    }
}

/// POST /api/contracts/:address/verify
pub async fn verify_contract(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Json(req): Json<VerifyRequest>,
) -> ApiResult<(StatusCode, Json<VerifyResponse>)> {
    let address = normalize_address(&address);

    // Validate compiler version format: v<major>.<minor>.<patch>+commit.<hex>
    validate_compiler_version(&req.compiler_version)?;

    // Validate that exactly one source input is provided
    let is_multi_file = match (&req.source_code, &req.source_files) {
        (Some(_), None) => false,
        (None, Some(_)) => true,
        _ => {
            return Err(AtlasError::InvalidInput(
                "provide either source_code or source_files, not both".to_string(),
            )
            .into())
        }
    };

    // Ensure the address is a known contract
    let is_contract: Option<(bool,)> =
        sqlx::query_as("SELECT is_contract FROM addresses WHERE address = $1")
            .bind(&address)
            .fetch_optional(&state.pool)
            .await?;

    match is_contract {
        Some((true,)) => {}
        Some((false,)) => {
            return Err(
                AtlasError::Verification(format!("{address} is not a contract address")).into(),
            )
        }
        None => return Err(AtlasError::NotFound(format!("address {address} not found")).into()),
    }

    // Reject if already verified
    let already_verified: Option<(String,)> =
        sqlx::query_as("SELECT address FROM contract_abis WHERE address = $1")
            .bind(&address)
            .fetch_optional(&state.pool)
            .await?;
    if already_verified.is_some() {
        return Err(AtlasError::Verification(format!("{address} is already verified")).into());
    }

    // Fetch deployed bytecode from the RPC node
    let deployed_hex = fetch_deployed_bytecode(&state.rpc_url, &address).await?;
    if deployed_hex == "0x" || deployed_hex.is_empty() {
        return Err(
            AtlasError::Verification("no bytecode deployed at this address".to_string()).into(),
        );
    }

    // Get the solc binary (download if not cached)
    let solc_path = get_solc_binary(&req.compiler_version, &state.solc_cache_dir).await?;

    // Compile the submitted source
    let compiled_contract = compile_source(&solc_path, &req).await?;

    // Strip CBOR metadata from both sides before comparing
    let deployed_bytes = decode_hex_bytecode(&deployed_hex)?;
    let deployed_stripped = strip_metadata(&deployed_bytes);
    let compiled_stripped = strip_metadata(&compiled_contract.bytecode);
    let deployed_cmp = normalize_bytecode_for_comparison(
        deployed_stripped,
        &compiled_contract.immutable_references,
    )?;
    let compiled_cmp = normalize_bytecode_for_comparison(
        compiled_stripped,
        &compiled_contract.immutable_references,
    )?;

    // eth_getCode returns deployed runtime bytecode, so constructor args are not
    // part of the bytecode comparison. We still parse and persist them as metadata.
    if deployed_cmp != compiled_cmp {
        return Err(AtlasError::BytecodeMismatch(
            "compiled bytecode does not match on-chain bytecode".to_string(),
        )
        .into());
    }

    let constructor_bytes = parse_constructor_args(req.constructor_args.as_deref())?;
    let abi = compiled_contract.abi;

    // Upsert into contract_abis
    let constructor_args_bytes: Option<Vec<u8>> = if constructor_bytes.is_empty() {
        None
    } else {
        Some(constructor_bytes)
    };
    let optimization_runs = if req.optimization_enabled {
        req.optimization_runs.or(Some(200))
    } else {
        None
    };
    let source_files_json: Option<serde_json::Value> = req
        .source_files
        .as_ref()
        .map(|m| serde_json::to_value(m).unwrap_or(serde_json::Value::Null));

    sqlx::query(
        "INSERT INTO contract_abis
            (address, abi, source_code, compiler_version, optimization_used, runs,
             contract_name, constructor_args, evm_version, license_type,
             is_multi_file, source_files, verified_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW())
         ON CONFLICT (address) DO UPDATE SET
            abi = EXCLUDED.abi,
            source_code = EXCLUDED.source_code,
            compiler_version = EXCLUDED.compiler_version,
            optimization_used = EXCLUDED.optimization_used,
            runs = EXCLUDED.runs,
            contract_name = EXCLUDED.contract_name,
            constructor_args = EXCLUDED.constructor_args,
            evm_version = EXCLUDED.evm_version,
            license_type = EXCLUDED.license_type,
            is_multi_file = EXCLUDED.is_multi_file,
            source_files = EXCLUDED.source_files,
            verified_at = NOW()",
    )
    .bind(&address)
    .bind(&abi)
    .bind(&req.source_code)
    .bind(&req.compiler_version)
    .bind(req.optimization_enabled)
    .bind(optimization_runs)
    .bind(&req.contract_name)
    .bind(constructor_args_bytes)
    .bind(&req.evm_version)
    .bind(&req.license_type)
    .bind(is_multi_file)
    .bind(source_files_json)
    .execute(&state.pool)
    .await?;

    Ok((
        StatusCode::OK,
        Json(VerifyResponse {
            verified: true,
            abi,
        }),
    ))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}

fn validate_compiler_version(version: &str) -> Result<(), AtlasError> {
    // Expected format: v<major>.<minor>.<patch>+commit.<8-hex-chars>
    // Allow longer hex hashes too (some builds use more chars)
    let valid = version.starts_with('v')
        && version.contains("+commit.")
        && !version.contains('/')
        && !version.contains("..")
        && !version.contains(' ')
        && version.len() < 80;

    if !valid {
        return Err(AtlasError::InvalidInput(format!(
            "invalid compiler version format: {version}; expected e.g. v0.8.20+commit.a1b79de6"
        )));
    }
    Ok(())
}

/// Download (if needed) and return the path to the solc binary for `version`.
///
/// Uses an atomic write (temp → rename) so concurrent requests for the same
/// version don't corrupt the cached binary.
async fn get_solc_binary(version: &str, cache_dir: &str) -> Result<PathBuf, AtlasError> {
    let target = solc_binary_target(OS, ARCH)?;
    let filename = format!("solc-{target}-{version}");
    let cache_path = PathBuf::from(cache_dir).join(&filename);

    if cache_path.exists() {
        return Ok(cache_path);
    }

    // Ensure cache directory exists
    fs::create_dir_all(cache_dir)
        .await
        .map_err(|e| AtlasError::Internal(format!("failed to create solc cache dir: {e}")))?;

    let url = format!("https://binaries.soliditylang.org/{target}/{filename}");
    tracing::info!(version, url, "downloading solc binary");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| AtlasError::Internal(e.to_string()))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AtlasError::Internal(format!("failed to download solc: {e}")))?;

    if !resp.status().is_success() {
        return Err(AtlasError::Verification(format!(
            "solc version {version} not found at {url} (HTTP {})",
            resp.status()
        )));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AtlasError::Internal(format!("failed to read solc response: {e}")))?;

    // Write to a temp file first, then atomically rename
    let tmp_path = PathBuf::from(cache_dir).join(format!("{filename}.tmp"));
    let mut file = fs::File::create(&tmp_path)
        .await
        .map_err(|e| AtlasError::Internal(format!("failed to create temp solc file: {e}")))?;
    file.write_all(&bytes)
        .await
        .map_err(|e| AtlasError::Internal(format!("failed to write solc binary: {e}")))?;
    drop(file);

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&tmp_path, perms)
            .map_err(|e| AtlasError::Internal(format!("failed to chmod solc: {e}")))?;
    }

    // Atomic rename
    fs::rename(&tmp_path, &cache_path)
        .await
        .map_err(|e| AtlasError::Internal(format!("failed to install solc binary: {e}")))?;

    tracing::info!(version, path = %cache_path.display(), "solc binary ready");
    Ok(cache_path)
}

fn solc_binary_target(os: &str, arch: &str) -> Result<&'static str, AtlasError> {
    match (os, arch) {
        ("linux", "x86_64") => Ok("linux-amd64"),
        // Solidity's official static macOS binaries are currently published under
        // macosx-amd64. Apple Silicon can execute them natively via Rosetta.
        ("macos", "x86_64") | ("macos", "aarch64") => Ok("macosx-amd64"),
        _ => Err(AtlasError::Verification(format!(
            "unsupported platform for native solc download: {os}/{arch}. \
             Official Solidity static binaries are currently available for linux/x86_64 \
             and macOS. For Docker on Apple Silicon, run atlas-server as linux/amd64."
        ))),
    }
}

/// Compile source and return the deployed bytecode bytes for the specified contract.
async fn compile_standard_json(
    solc_path: &PathBuf,
    req: &VerifyRequest,
    dir: tempfile::TempDir,
) -> Result<serde_json::Value, AtlasError> {
    let input = build_standard_json_input(req, true)?;
    let input_str = serde_json::to_string(&input)
        .map_err(|e| AtlasError::Internal(format!("failed to serialize solc input: {e}")))?;

    let mut child = tokio::process::Command::new(solc_path)
        .arg("--standard-json")
        .current_dir(dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AtlasError::Internal(format!("failed to spawn solc: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt as _;
        stdin
            .write_all(input_str.as_bytes())
            .await
            .map_err(|e| AtlasError::Internal(format!("failed to write solc stdin: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| AtlasError::Internal(format!("failed to wait for solc: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| {
        let stderr_hint = if stderr.is_empty() {
            String::new()
        } else {
            format!("; stderr: {}", stderr.trim())
        };
        AtlasError::Internal(format!(
            "failed to parse solc output (exit={:?}): {e}{stderr_hint}",
            output.status.code()
        ))
    })?;

    collect_fatal_solc_errors(&json)?;
    Ok(json)
}

/// Compile submitted source and return runtime bytecode, ABI, and immutable refs.
async fn compile_source(
    solc_path: &PathBuf,
    req: &VerifyRequest,
) -> Result<CompiledContract, AtlasError> {
    let dir = tempfile::tempdir()
        .map_err(|e| AtlasError::Internal(format!("failed to create temp dir: {e}")))?;
    let json = compile_standard_json(solc_path, req, dir).await?;
    extract_compiled_contract(&json, &req.contract_name)
}

fn build_standard_json_input(
    req: &VerifyRequest,
    include_deployed_bytecode: bool,
) -> Result<serde_json::Value, AtlasError> {
    let sources = build_sources_json(req)?;
    let runs = req.optimization_runs.unwrap_or(200);
    let mut contract_outputs = vec![serde_json::json!("abi")];
    if include_deployed_bytecode {
        contract_outputs.push(serde_json::json!("evm.deployedBytecode"));
    }

    Ok(serde_json::json!({
        "language": "Solidity",
        "sources": sources,
        "settings": {
            "optimizer": {
                "enabled": req.optimization_enabled,
                "runs": runs,
            },
            "evmVersion": req.evm_version.as_deref().unwrap_or("default"),
            "outputSelection": {
                "*": { "*": contract_outputs }
            }
        }
    }))
}

fn build_sources_json(req: &VerifyRequest) -> Result<serde_json::Value, AtlasError> {
    let files: HashMap<String, String> = match (&req.source_code, &req.source_files) {
        (Some(source), None) => HashMap::from([("contract.sol".to_string(), source.clone())]),
        (None, Some(files)) => files.clone(),
        _ => {
            return Err(AtlasError::InvalidInput(
                "provide either source_code or source_files, not both".to_string(),
            ))
        }
    };

    let sources = files
        .into_iter()
        .map(|(path, content)| (path, serde_json::json!({ "content": content })))
        .collect::<serde_json::Map<String, serde_json::Value>>();
    Ok(serde_json::Value::Object(sources))
}

fn collect_fatal_solc_errors(json: &serde_json::Value) -> Result<(), AtlasError> {
    let fatal: Vec<String> = json
        .get("errors")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .filter(|e| e.get("severity").and_then(|s| s.as_str()) == Some("error"))
        .filter_map(|e| {
            e.get("formattedMessage")
                .and_then(|m| m.as_str())
                .map(String::from)
        })
        .collect();
    if fatal.is_empty() {
        Ok(())
    } else {
        Err(AtlasError::Compilation(fatal.join("\n")))
    }
}

fn extract_compiled_contract(
    json: &serde_json::Value,
    contract_name: &str,
) -> Result<CompiledContract, AtlasError> {
    let contracts = json
        .get("contracts")
        .and_then(|c| c.as_object())
        .ok_or_else(|| AtlasError::Compilation("no contracts in solc output".to_string()))?;

    for (_file, file_contracts) in contracts {
        if let Some(contract) = file_contracts.get(contract_name) {
            let bytecode = contract
                .pointer("/evm/deployedBytecode/object")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AtlasError::Compilation(format!(
                        "no deployedBytecode for contract {contract_name}"
                    ))
                })?;
            if bytecode.is_empty() {
                return Err(AtlasError::Compilation(format!(
                    "empty deployed bytecode for {contract_name} — is it abstract?"
                )));
            }

            let abi = contract
                .get("abi")
                .cloned()
                .ok_or_else(|| AtlasError::Compilation("no abi in solc output".to_string()))?;
            let immutable_references = contract
                .pointer("/evm/deployedBytecode/immutableReferences")
                .map(extract_immutable_references)
                .transpose()?
                .unwrap_or_default();

            return Ok(CompiledContract {
                bytecode: decode_hex_bytecode(&format!("0x{bytecode}"))?,
                abi,
                immutable_references,
            });
        }
    }

    Err(AtlasError::Compilation(format!(
        "contract {contract_name} not found in solc output"
    )))
}

/// Strip CBOR-encoded metadata suffix from EVM bytecode.
///
/// Solc appends a CBOR blob; the last 2 bytes encode its length (big-endian u16).
/// We strip `metadata_len + 2` bytes from the end.
pub fn strip_metadata(bytecode: &[u8]) -> &[u8] {
    if bytecode.len() < 2 {
        return bytecode;
    }
    let meta_len =
        u16::from_be_bytes([bytecode[bytecode.len() - 2], bytecode[bytecode.len() - 1]]) as usize;
    let total_strip = meta_len + 2;
    if total_strip > bytecode.len() {
        return bytecode;
    }
    &bytecode[..bytecode.len() - total_strip]
}

fn normalize_bytecode_for_comparison(
    bytecode: &[u8],
    immutable_references: &[ImmutableReference],
) -> Result<Vec<u8>, AtlasError> {
    let mut normalized = bytecode.to_vec();

    for reference in immutable_references {
        let end = reference.start.saturating_add(reference.length);
        if end > normalized.len() {
            return Err(AtlasError::Compilation(format!(
                "immutable reference out of bounds: start={}, length={}, bytecode_len={}",
                reference.start,
                reference.length,
                normalized.len()
            )));
        }
        normalized[reference.start..end].fill(0);
    }

    Ok(normalized)
}

fn extract_immutable_references(
    value: &serde_json::Value,
) -> Result<Vec<ImmutableReference>, AtlasError> {
    let mut refs = Vec::new();
    let Some(map) = value.as_object() else {
        return Err(AtlasError::Compilation(
            "invalid immutableReferences in solc output".to_string(),
        ));
    };

    for entries in map.values() {
        let Some(entries) = entries.as_array() else {
            return Err(AtlasError::Compilation(
                "invalid immutableReferences entry in solc output".to_string(),
            ));
        };
        for entry in entries {
            let start = entry.get("start").and_then(|v| v.as_u64()).ok_or_else(|| {
                AtlasError::Compilation(
                    "missing immutable reference start in solc output".to_string(),
                )
            })? as usize;
            let length = entry
                .get("length")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    AtlasError::Compilation(
                        "missing immutable reference length in solc output".to_string(),
                    )
                })? as usize;
            refs.push(ImmutableReference { start, length });
        }
    }

    Ok(refs)
}

/// Decode a hex-encoded bytecode string (with or without 0x prefix) to bytes.
fn decode_hex_bytecode(hex_str: &str) -> Result<Vec<u8>, AtlasError> {
    let stripped = hex_str.trim_start_matches("0x");
    hex::decode(stripped)
        .map_err(|e| AtlasError::Internal(format!("invalid hex bytecode from RPC: {e}")))
}

/// Decode optional hex constructor args string.
fn parse_constructor_args(args: Option<&str>) -> Result<Vec<u8>, AtlasError> {
    match args {
        None | Some("") => Ok(vec![]),
        Some(s) => {
            let stripped = s.trim_start_matches("0x");
            hex::decode(stripped)
                .map_err(|e| AtlasError::InvalidInput(format!("invalid constructor_args hex: {e}")))
        }
    }
}

/// Call eth_getCode on the configured RPC to get the deployed bytecode.
async fn fetch_deployed_bytecode(rpc_url: &str, address: &str) -> Result<String, AtlasError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getCode",
        "params": [address, "latest"],
        "id": 1
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AtlasError::Internal(e.to_string()))?;

    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AtlasError::Rpc(format!("eth_getCode failed: {e}")))?
        .json()
        .await
        .map_err(|e| AtlasError::Rpc(format!("failed to parse eth_getCode response: {e}")))?;

    resp.get("result")
        .and_then(|r| r.as_str())
        .map(String::from)
        .ok_or_else(|| AtlasError::Rpc("eth_getCode returned no result".to_string()))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_metadata_removes_cbor_suffix() {
        // Simulate bytecode with a 3-byte CBOR blob + 2-byte length header
        // Last 2 bytes = 0x00 0x03 (length = 3), preceded by 3 bytes of "metadata"
        let bytecode: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xAA, 0xBB, 0xCC, 0x00, 0x03];
        let stripped = strip_metadata(&bytecode);
        assert_eq!(stripped, &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn strip_metadata_handles_too_short_bytecode() {
        let bytecode: Vec<u8> = vec![0xDE];
        let stripped = strip_metadata(&bytecode);
        assert_eq!(stripped, &[0xDE]);
    }

    #[test]
    fn strip_metadata_handles_oversized_length() {
        // If the claimed metadata length is larger than the bytecode, return as-is
        let bytecode: Vec<u8> = vec![0xDE, 0xAD, 0xFF, 0xFF]; // length = 65535, way too big
        let stripped = strip_metadata(&bytecode);
        assert_eq!(stripped, &[0xDE, 0xAD, 0xFF, 0xFF]);
    }

    #[test]
    fn validate_compiler_version_accepts_valid() {
        assert!(validate_compiler_version("v0.8.20+commit.a1b79de6").is_ok());
        assert!(validate_compiler_version("v0.7.6+commit.7338295f").is_ok());
        assert!(validate_compiler_version("v0.6.12+commit.27d51765").is_ok());
    }

    #[test]
    fn validate_compiler_version_rejects_invalid() {
        assert!(validate_compiler_version("0.8.20+commit.a1b79de6").is_err()); // missing v
        assert!(validate_compiler_version("v0.8.20").is_err()); // missing +commit
        assert!(validate_compiler_version("../evil/path").is_err()); // path traversal
        assert!(validate_compiler_version("v0.8.20 extra").is_err()); // space
    }

    #[test]
    fn parse_constructor_args_empty() {
        assert_eq!(parse_constructor_args(None).unwrap(), Vec::<u8>::new());
        assert_eq!(parse_constructor_args(Some("")).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn parse_constructor_args_with_prefix() {
        assert_eq!(
            parse_constructor_args(Some("0xdeadbeef")).unwrap(),
            vec![0xde_u8, 0xad, 0xbe, 0xef]
        );
    }

    #[test]
    fn solc_binary_target_supports_linux_amd64() {
        assert_eq!(
            solc_binary_target("linux", "x86_64").unwrap(),
            "linux-amd64"
        );
    }

    #[test]
    fn solc_binary_target_supports_macos_arm64_via_rosetta() {
        assert_eq!(
            solc_binary_target("macos", "aarch64").unwrap(),
            "macosx-amd64"
        );
    }

    #[test]
    fn solc_binary_target_rejects_linux_arm64() {
        let err = solc_binary_target("linux", "aarch64").unwrap_err();
        assert!(matches!(err, AtlasError::Verification(_)));
    }

    #[test]
    fn normalize_bytecode_for_comparison_zeroes_immutable_ranges() {
        let bytecode = vec![0xaa, 0xbb, 0x01, 0x02, 0x03, 0xcc];
        let normalized = normalize_bytecode_for_comparison(
            &bytecode,
            &[ImmutableReference {
                start: 2,
                length: 3,
            }],
        )
        .unwrap();
        assert_eq!(normalized, vec![0xaa, 0xbb, 0x00, 0x00, 0x00, 0xcc]);
    }

    #[test]
    fn normalize_bytecode_for_comparison_rejects_out_of_bounds_ranges() {
        let err = normalize_bytecode_for_comparison(
            &[0xaa, 0xbb],
            &[ImmutableReference {
                start: 1,
                length: 4,
            }],
        )
        .unwrap_err();
        assert!(matches!(err, AtlasError::Compilation(_)));
    }

    #[test]
    fn extract_immutable_references_parses_multiple_entries() {
        let refs = extract_immutable_references(&serde_json::json!({
            "3": [{ "start": 5, "length": 32 }],
            "8": [{ "start": 80, "length": 32 }, { "start": 144, "length": 20 }]
        }))
        .unwrap();

        assert_eq!(
            refs,
            vec![
                ImmutableReference {
                    start: 5,
                    length: 32,
                },
                ImmutableReference {
                    start: 80,
                    length: 32,
                },
                ImmutableReference {
                    start: 144,
                    length: 20,
                },
            ]
        );
    }
}
