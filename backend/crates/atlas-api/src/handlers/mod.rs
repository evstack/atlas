pub mod blocks;
pub mod transactions;
pub mod addresses;
pub mod nfts;
pub mod search;
pub mod tokens;
pub mod logs;
pub mod etherscan;
pub mod labels;
pub mod proxy;
pub mod contracts;
pub mod status;

use sqlx::PgPool;

/// Get table row count efficiently.
/// - For tables > 100k rows: uses PostgreSQL's approximate count (instant, ~99% accurate)
/// - For smaller tables: uses exact COUNT(*) (fast enough)
///
/// This avoids the slow COUNT(*) full table scan on large tables.
pub async fn get_table_count(pool: &PgPool, table_name: &str) -> Result<i64, sqlx::Error> {
    // First get the approximate count from pg_class (instant)
    let approx: (Option<f32>,) = sqlx::query_as(
        "SELECT reltuples FROM pg_class WHERE relname = $1"
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    let approx_count = approx.0.unwrap_or(0.0) as i64;

    // If table is large (> 100k rows), use approximate count
    // Otherwise, use exact count (fast enough for small tables)
    if approx_count > 100_000 {
        Ok(approx_count)
    } else {
        // Safe to use exact count for smaller tables
        let exact: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table_name))
            .fetch_one(pool)
            .await?;
        Ok(exact.0)
    }
}

/// Get count with a WHERE clause efficiently.
/// Uses exact count since filtered queries are usually fast with proper indexes.
pub async fn get_filtered_count(pool: &PgPool, query: &str) -> Result<i64, sqlx::Error> {
    let count: (i64,) = sqlx::query_as(query)
        .fetch_one(pool)
        .await?;
    Ok(count.0)
}
