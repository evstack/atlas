use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use atlas_common::{AtlasError, EventLog, Pagination, PaginatedResponse};
use crate::AppState;
use crate::error::ApiResult;

/// Query parameters for log filtering
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    /// Filter by event signature (topic0)
    pub topic0: Option<String>,
    /// Optional pagination
    #[serde(flatten)]
    pub pagination: Pagination,
}

/// GET /api/transactions/:hash/logs - Get all logs for a transaction
pub async fn get_transaction_logs(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> ApiResult<Json<Vec<EventLog>>> {
    let hash = normalize_hash(&hash);

    let logs: Vec<EventLog> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number, decoded
         FROM event_logs
         WHERE LOWER(tx_hash) = LOWER($1)
         ORDER BY log_index ASC",
    )
    .bind(&hash)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(logs))
}

/// GET /api/addresses/:address/logs - Get logs emitted by a contract
pub async fn get_address_logs(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(query): Query<LogsQuery>,
) -> ApiResult<Json<PaginatedResponse<EventLog>>> {
    let address = normalize_address(&address);

    let (total, logs) = if let Some(topic0) = &query.topic0 {
        let topic0 = normalize_hash(topic0);

        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM event_logs WHERE address = $1 AND topic0 = $2",
        )
        .bind(&address)
        .bind(&topic0)
        .fetch_one(&state.pool)
        .await?;

        let logs: Vec<EventLog> = sqlx::query_as(
            "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number, decoded
             FROM event_logs
             WHERE address = $1 AND topic0 = $2
             ORDER BY block_number DESC, log_index DESC
             LIMIT $3 OFFSET $4",
        )
        .bind(&address)
        .bind(&topic0)
        .bind(query.pagination.limit())
        .bind(query.pagination.offset())
        .fetch_all(&state.pool)
        .await?;

        (total.0, logs)
    } else {
        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM event_logs WHERE address = $1",
        )
        .bind(&address)
        .fetch_one(&state.pool)
        .await?;

        let logs: Vec<EventLog> = sqlx::query_as(
            "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number, decoded
             FROM event_logs
             WHERE address = $1
             ORDER BY block_number DESC, log_index DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(&address)
        .bind(query.pagination.limit())
        .bind(query.pagination.offset())
        .fetch_all(&state.pool)
        .await?;

        (total.0, logs)
    };

    Ok(Json(PaginatedResponse::new(
        logs,
        query.pagination.page,
        query.pagination.limit,
        total,
    )))
}

/// GET /api/logs - Filter logs by topic0 (event signature)
pub async fn get_logs_by_topic(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LogsQuery>,
) -> ApiResult<Json<PaginatedResponse<EventLog>>> {
    let topic0 = query
        .topic0
        .as_ref()
        .ok_or_else(|| AtlasError::InvalidInput("topic0 query parameter is required".to_string()))?;
    let topic0 = normalize_hash(topic0);

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM event_logs WHERE topic0 = $1",
    )
    .bind(&topic0)
    .fetch_one(&state.pool)
    .await?;

    let logs: Vec<EventLog> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number, decoded
         FROM event_logs
         WHERE topic0 = $1
         ORDER BY block_number DESC, log_index DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(&topic0)
    .bind(query.pagination.limit())
    .bind(query.pagination.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        logs,
        query.pagination.page,
        query.pagination.limit,
        total.0,
    )))
}

/// Enriched log with event name
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrichedEventLog {
    #[serde(flatten)]
    pub log: EventLog,
    pub event_name: Option<String>,
    pub event_signature: Option<String>,
}

/// GET /api/transactions/:hash/logs/decoded - Get decoded logs for a transaction
pub async fn get_transaction_logs_decoded(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> ApiResult<Json<Vec<EnrichedEventLog>>> {
    let hash = normalize_hash(&hash);

    let logs: Vec<EventLog> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number, decoded
         FROM event_logs
         WHERE LOWER(tx_hash) = LOWER($1)
         ORDER BY log_index ASC",
    )
    .bind(&hash)
    .fetch_all(&state.pool)
    .await?;

    // Collect unique topic0 values for signature lookup
    let topic0s: Vec<String> = logs.iter().map(|l| l.topic0.clone()).collect();

    // Fetch known event signatures
    let signatures: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT signature, name, full_signature FROM event_signatures WHERE signature = ANY($1)",
    )
    .bind(&topic0s)
    .fetch_all(&state.pool)
    .await?;

    let sig_map: std::collections::HashMap<String, (String, String)> = signatures
        .into_iter()
        .map(|(sig, name, full)| (sig.to_lowercase(), (name, full)))
        .collect();

    let enriched: Vec<EnrichedEventLog> = logs
        .into_iter()
        .map(|log| {
            let (event_name, event_signature) = sig_map
                .get(&log.topic0.to_lowercase())
                .map(|(n, s)| (Some(n.clone()), Some(s.clone())))
                .unwrap_or((None, None));
            EnrichedEventLog {
                log,
                event_name,
                event_signature,
            }
        })
        .collect();

    Ok(Json(enriched))
}

fn normalize_hash(hash: &str) -> String {
    if hash.starts_with("0x") {
        hash.to_lowercase()
    } else {
        format!("0x{}", hash.to_lowercase())
    }
}

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_lowercase()
    } else {
        format!("0x{}", address.to_lowercase())
    }
}
