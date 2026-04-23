pub mod addresses;
pub mod blocks;
pub mod config;
pub mod contracts;
pub mod etherscan;
pub mod faucet;
pub mod health;
pub mod logs;
pub mod metrics;
pub mod nfts;
pub mod proxy;
pub mod search;
pub mod sse;
pub mod stats;
pub mod status;
pub mod tokens;
pub mod transactions;

use atlas_common::{Block, BLOCK_COLUMNS};
use sqlx::PgPool;

use crate::state_keys::ERC20_SUPPLY_HISTORY_COMPLETE_KEY;

pub async fn get_latest_block(pool: &PgPool) -> Result<Option<Block>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT {} FROM blocks ORDER BY number DESC LIMIT 1",
        BLOCK_COLUMNS
    ))
    .fetch_optional(pool)
    .await
}

pub async fn has_complete_erc20_supply_history(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let value: Option<(String,)> =
        sqlx::query_as("SELECT value FROM indexer_state WHERE key = $1 LIMIT 1")
            .bind(ERC20_SUPPLY_HISTORY_COMPLETE_KEY)
            .fetch_optional(pool)
            .await?;

    Ok(matches!(
        value.as_ref().map(|(value,)| value.as_str()),
        Some("true")
    ))
}
fn exact_count_sql(table_name: &str) -> Result<&'static str, sqlx::Error> {
    match table_name {
        "transactions" => Ok("SELECT COUNT(*) FROM transactions"),
        "addresses" => Ok("SELECT COUNT(*) FROM addresses"),
        _ => Err(sqlx::Error::Protocol(format!(
            "unsupported table for exact count: {table_name}"
        ))),
    }
}

fn should_use_approximate_count(approx: i64) -> bool {
    approx > 100_000
}

/// Get a table's row count efficiently.
/// - For tables > 100k rows: uses PostgreSQL's approximate count (instant, ~99% accurate)
/// - For smaller tables: uses exact COUNT(*) (fast enough)
///
/// This avoids the slow COUNT(*) full table scan on large tables.
pub async fn get_table_count(pool: &PgPool, table_name: &str) -> Result<i64, sqlx::Error> {
    // Sum approximate reltuples across partitions if any, else use parent.
    // This is instant and reasonably accurate for large tables.
    // Cast to float8 (f64) since reltuples is float4 and SUM returns float4
    let approx_partitions: (Option<f64>,) = sqlx::query_as(
        r#"
        SELECT SUM(c.reltuples)::float8 AS approx
        FROM pg_class c
        JOIN pg_inherits i ON i.inhrelid = c.oid
        JOIN pg_class p ON p.oid = i.inhparent
        WHERE p.relname = $1
        "#,
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    let approx = if let Some(sum) = approx_partitions.0 {
        sum as i64
    } else {
        let parent: (Option<f64>,) =
            sqlx::query_as("SELECT reltuples::float8 FROM pg_class WHERE relname = $1")
                .bind(table_name)
                .fetch_one(pool)
                .await?;
        parent.0.unwrap_or(0.0) as i64
    };

    if should_use_approximate_count(approx) {
        Ok(approx)
    } else {
        // Exact count for small tables
        let exact: (i64,) = sqlx::query_as(exact_count_sql(table_name)?)
            .fetch_one(pool)
            .await?;
        Ok(exact.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_count_sql_whitelists_supported_tables() {
        assert_eq!(
            exact_count_sql("transactions").unwrap(),
            "SELECT COUNT(*) FROM transactions"
        );
        assert_eq!(
            exact_count_sql("addresses").unwrap(),
            "SELECT COUNT(*) FROM addresses"
        );
    }

    #[test]
    fn exact_count_sql_rejects_unsupported_tables() {
        let err = exact_count_sql("blocks").unwrap_err();
        assert!(err.to_string().contains("unsupported table"));
    }

    #[test]
    fn should_use_approximate_count_above_threshold() {
        assert!(should_use_approximate_count(100_001));
    }

    #[test]
    fn should_use_approximate_count_uses_exact_count_at_threshold_and_below() {
        assert!(!should_use_approximate_count(100_000));
        assert!(!should_use_approximate_count(42));
    }
}
