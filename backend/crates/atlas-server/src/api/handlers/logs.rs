use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::api::error::ApiResult;
use crate::api::AppState;
use crate::event_log_decode::EventLogApiResponse;
use atlas_common::{EventLog, PaginatedResponse};

/// Pagination for transaction log endpoints.
#[derive(Debug, Deserialize)]
pub struct TransactionLogsQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

impl TransactionLogsQuery {
    fn clamped_limit(&self) -> u32 {
        self.limit.min(100)
    }

    fn offset(&self) -> i64 {
        (self.page.saturating_sub(1) as i64) * self.clamped_limit() as i64
    }

    fn limit(&self) -> i64 {
        self.clamped_limit() as i64
    }
}

/// Query parameters for log filtering
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    /// Filter by event signature (topic0)
    pub topic0: Option<String>,
    /// Optional pagination
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

impl LogsQuery {
    fn clamped_limit(&self) -> u32 {
        self.limit.min(100)
    }

    fn offset(&self) -> i64 {
        (self.page.saturating_sub(1) as i64) * self.clamped_limit() as i64
    }

    fn limit(&self) -> i64 {
        self.clamped_limit() as i64
    }
}

/// GET /api/transactions/:hash/logs - Get all logs for a transaction
pub async fn get_transaction_logs(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Query(query): Query<TransactionLogsQuery>,
) -> ApiResult<Json<PaginatedResponse<EventLogApiResponse>>> {
    let hash = normalize_hash(&hash);

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM event_logs WHERE tx_hash = $1")
        .bind(&hash)
        .fetch_one(&state.pool)
        .await?;

    let logs: Vec<EventLog> = sqlx::query_as(
        "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number,
                decoded, decode_status, decoded_at, decode_attempted_at, decode_source
         FROM event_logs
         WHERE tx_hash = $1
         ORDER BY log_index ASC
         LIMIT $2 OFFSET $3",
    )
    .bind(&hash)
    .bind(query.limit())
    .bind(query.offset())
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(PaginatedResponse::new(
        logs.iter().map(EventLogApiResponse::from).collect(),
        query.page,
        query.clamped_limit(),
        total.0,
    )))
}

/// GET /api/addresses/:address/logs - Get logs emitted by a contract
pub async fn get_address_logs(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(query): Query<LogsQuery>,
) -> ApiResult<Json<PaginatedResponse<EventLogApiResponse>>> {
    let address = normalize_address(&address);

    let (total, logs) = if let Some(topic0) = &query.topic0 {
        let topic0 = normalize_hash(topic0);

        let total: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM event_logs WHERE address = $1 AND topic0 = $2")
                .bind(&address)
                .bind(&topic0)
                .fetch_one(&state.pool)
                .await?;

        let logs: Vec<EventLog> = sqlx::query_as(
            "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number,
                    decoded, decode_status, decoded_at, decode_attempted_at, decode_source
             FROM event_logs
             WHERE address = $1 AND topic0 = $2
             ORDER BY block_number DESC, log_index DESC
             LIMIT $3 OFFSET $4",
        )
        .bind(&address)
        .bind(&topic0)
        .bind(query.limit())
        .bind(query.offset())
        .fetch_all(&state.pool)
        .await?;

        (total.0, logs)
    } else {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM event_logs WHERE address = $1")
            .bind(&address)
            .fetch_one(&state.pool)
            .await?;

        let logs: Vec<EventLog> = sqlx::query_as(
            "SELECT id, tx_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number,
                    decoded, decode_status, decoded_at, decode_attempted_at, decode_source
             FROM event_logs
             WHERE address = $1
             ORDER BY block_number DESC, log_index DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(&address)
        .bind(query.limit())
        .bind(query.offset())
        .fetch_all(&state.pool)
        .await?;

        (total.0, logs)
    };

    Ok(Json(PaginatedResponse::new(
        logs.iter().map(EventLogApiResponse::from).collect(),
        query.page,
        query.clamped_limit(),
        total,
    )))
}

/// GET /api/transactions/:hash/logs/decoded - Get decoded logs for a transaction
pub async fn get_transaction_logs_decoded(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
    Query(query): Query<TransactionLogsQuery>,
) -> ApiResult<Json<PaginatedResponse<EventLogApiResponse>>> {
    get_transaction_logs(State(state), Path(hash), Query(query)).await
}

fn default_page() -> u32 {
    1
}

fn default_limit() -> u32 {
    20
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

#[cfg(test)]
mod tests {
    use super::LogsQuery;
    use super::TransactionLogsQuery;
    use atlas_common::PaginatedResponse;

    #[test]
    fn transaction_logs_query_clamps_limit_for_offset_and_metadata() {
        let query = TransactionLogsQuery {
            page: 2,
            limit: 1000,
        };

        assert_eq!(query.clamped_limit(), 100);
        assert_eq!(query.offset(), 100);
        assert_eq!(query.limit(), 100);

        let response =
            PaginatedResponse::new(Vec::<()>::new(), query.page, query.clamped_limit(), 250);
        assert_eq!(response.limit, 100);
        assert_eq!(response.total_pages, 3);
    }

    #[test]
    fn logs_query_parses_pagination_from_query_string() {
        use axum::extract::Query;
        use axum::http::Uri;

        let uri: Uri = "/?limit=20&page=2".parse().unwrap();
        let Query(q): Query<LogsQuery> =
            Query::try_from_uri(&uri).expect("Should be able to parse the query string");

        assert_eq!(q.page, 2);
        assert_eq!(q.limit, 20);

        assert_eq!(q.limit(), 20);
        assert_eq!(q.offset(), 20);
    }
}
