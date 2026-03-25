use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::error::ApiResult;
use crate::api::AppState;

/// Time window for chart queries.
#[derive(Deserialize, Default, Clone, Copy)]
pub enum Window {
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "6h")]
    SixHours,
    #[default]
    #[serde(rename = "24h")]
    TwentyFourHours,
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "1m")]
    OneMonth,
    #[serde(rename = "6m")]
    SixMonths,
    #[serde(rename = "1y")]
    OneYear,
}

impl Window {
    pub fn duration_secs(self) -> i64 {
        match self {
            Window::OneHour => 3_600,
            Window::SixHours => 6 * 3_600,
            Window::TwentyFourHours => 24 * 3_600,
            Window::SevenDays => 7 * 24 * 3_600,
            Window::OneMonth => 30 * 24 * 3_600,
            Window::SixMonths => 180 * 24 * 3_600,
            Window::OneYear => 365 * 24 * 3_600,
        }
    }

    pub fn bucket_secs(self) -> i64 {
        match self {
            Window::OneHour => 300,             // 5-min buckets → 12 points
            Window::SixHours => 1_800,          // 30-min buckets → 12 points
            Window::TwentyFourHours => 3_600,   // 1-hour buckets → 24 points
            Window::SevenDays => 43_200,        // 12-hour buckets → 14 points
            Window::OneMonth => 86_400,         // 1-day buckets → 30 points
            Window::SixMonths => 7 * 86_400,    // 1-week buckets → ~26 points
            Window::OneYear => 14 * 86_400,     // 2-week buckets → ~26 points
        }
    }
}

#[derive(Deserialize)]
pub struct WindowQuery {
    #[serde(default)]
    pub window: Window,
}

#[derive(Serialize)]
pub struct BlockChartPoint {
    pub bucket: String,
    pub tx_count: i64,
    pub avg_gas_used: f64,
}

#[derive(Serialize)]
pub struct DailyTxPoint {
    pub day: String,
    pub tx_count: i64,
}

#[derive(Serialize)]
pub struct GasPricePoint {
    pub bucket: String,
    pub avg_gas_price: f64,
}

/// GET /api/stats/blocks-chart?window=1h|6h|24h|7d
///
/// Returns tx count and avg gas utilization bucketed over the given window.
/// Both metrics come from the `blocks` table so a single query serves both charts.
/// The window is anchored to the latest indexed block timestamp (not NOW()) so
/// charts show data even when the indexer is behind the live chain head.
pub async fn get_blocks_chart(
    State(state): State<Arc<AppState>>,
    Query(params): Query<WindowQuery>,
) -> ApiResult<Json<Vec<BlockChartPoint>>> {
    let window = params.window;
    let bucket_secs = window.bucket_secs();

    // Anchor to the latest indexed block timestamp, not wall-clock NOW().
    // This ensures charts always show data regardless of how far the indexer
    // is behind the live chain head.
    let rows: Vec<(chrono::DateTime<Utc>, i64, f64)> = sqlx::query_as(
        r#"
        WITH latest AS (SELECT MAX(timestamp) AS max_ts FROM blocks),
        agg AS (
            SELECT
                (timestamp - (timestamp % $1))::bigint AS bucket_ts,
                SUM(transaction_count)::bigint          AS tx_count,
                COALESCE(AVG(gas_used::float8), 0.0)   AS avg_gas_used
            FROM blocks, latest
            WHERE timestamp >= max_ts - $2
              AND timestamp <= max_ts
            GROUP BY 1
        )
        SELECT
            to_timestamp(gs::float8)                    AS bucket,
            COALESCE(a.tx_count, 0)::bigint             AS tx_count,
            COALESCE(a.avg_gas_used, 0.0)               AS avg_gas_used
        FROM generate_series(
            (SELECT (max_ts - $2) - ((max_ts - $2) % $1) FROM latest),
            (SELECT max_ts - (max_ts % $1) FROM latest),
            $1::bigint
        ) AS gs
        LEFT JOIN agg a ON a.bucket_ts = gs
        ORDER BY gs ASC
        "#,
    )
    .bind(bucket_secs)
    .bind(window.duration_secs())
    .fetch_all(&state.pool)
    .await?;

    let points = rows
        .into_iter()
        .map(|(bucket, tx_count, avg_gas_used)| BlockChartPoint {
            bucket: bucket.to_rfc3339(),
            tx_count,
            avg_gas_used,
        })
        .collect();

    Ok(Json(points))
}

/// GET /api/stats/daily-txs
///
/// Returns transaction counts per day for the last 14 days. Fixed window.
pub async fn get_daily_txs(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<DailyTxPoint>>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        WITH latest AS (SELECT MAX(timestamp) AS max_ts FROM transactions)
        SELECT
            to_char(to_timestamp(timestamp)::date, 'YYYY-MM-DD') AS day,
            COUNT(*)::bigint                                      AS tx_count
        FROM transactions, latest
        WHERE timestamp >= max_ts - (14 * 86400)
        GROUP BY to_timestamp(timestamp)::date
        ORDER BY to_timestamp(timestamp)::date ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let points = rows
        .into_iter()
        .map(|(day, tx_count)| DailyTxPoint { day, tx_count })
        .collect();

    Ok(Json(points))
}

/// GET /api/stats/gas-price?window=1h|6h|24h|7d
///
/// Returns average gas price (in wei) per bucket over the given window.
/// Anchored to the latest indexed transaction timestamp (not NOW()).
pub async fn get_gas_price_chart(
    State(state): State<Arc<AppState>>,
    Query(params): Query<WindowQuery>,
) -> ApiResult<Json<Vec<GasPricePoint>>> {
    let window = params.window;
    let bucket_secs = window.bucket_secs();

    let rows: Vec<(chrono::DateTime<Utc>, Option<f64>)> = sqlx::query_as(
        r#"
        WITH latest AS (SELECT MAX(timestamp) AS max_ts FROM blocks),
        agg AS (
            SELECT
                (timestamp - (timestamp % $1))::bigint AS bucket_ts,
                AVG(gas_price::float8)                  AS avg_gas_price
            FROM transactions, latest
            WHERE timestamp >= max_ts - $2
              AND timestamp <= max_ts
              AND gas_price > 0
            GROUP BY 1
        )
        SELECT
            to_timestamp(gs::float8) AS bucket,
            a.avg_gas_price
        FROM generate_series(
            (SELECT (max_ts - $2) - ((max_ts - $2) % $1) FROM latest),
            (SELECT max_ts - (max_ts % $1) FROM latest),
            $1::bigint
        ) AS gs
        LEFT JOIN agg a ON a.bucket_ts = gs
        ORDER BY gs ASC
        "#,
    )
    .bind(bucket_secs)
    .bind(window.duration_secs())
    .fetch_all(&state.pool)
    .await?;

    let points = rows
        .into_iter()
        .filter_map(|(bucket, avg_gas_price)| {
            avg_gas_price.map(|p| GasPricePoint {
                bucket: bucket.to_rfc3339(),
                avg_gas_price: p,
            })
        })
        .collect();

    Ok(Json(points))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_duration_secs() {
        assert_eq!(Window::OneHour.duration_secs(), 3_600);
        assert_eq!(Window::SixHours.duration_secs(), 6 * 3_600);
        assert_eq!(Window::TwentyFourHours.duration_secs(), 24 * 3_600);
        assert_eq!(Window::SevenDays.duration_secs(), 7 * 24 * 3_600);
    }

    #[test]
    fn window_bucket_secs_gives_reasonable_point_counts() {
        // Each window should yield ~12-28 data points
        for (window, expected_points) in [
            (Window::OneHour, 12),
            (Window::SixHours, 12),
            (Window::TwentyFourHours, 24),
            (Window::SevenDays, 14),
            (Window::OneMonth, 30),
            (Window::SixMonths, 25),
            (Window::OneYear, 26),
        ] {
            let points = window.duration_secs() / window.bucket_secs();
            assert_eq!(points, expected_points);
        }
    }

    #[test]
    fn gas_price_window_supports_7d() {
        // SevenDays is now supported for gas price queries
        assert_eq!(Window::SevenDays.duration_secs(), 7 * 24 * 3_600);
        assert_eq!(Window::SevenDays.bucket_secs(), 43_200);
    }
}
