pub mod addresses;
pub mod auth;
pub mod blocks;
pub mod contracts;
pub mod etherscan;
pub mod labels;
pub mod logs;
pub mod nfts;
pub mod proxy;
pub mod search;
pub mod status;
pub mod tokens;
pub mod transactions;

use sqlx::PgPool;

/// Get transactions table row count efficiently.
/// - For tables > 100k rows: uses PostgreSQL's approximate count (instant, ~99% accurate)
/// - For smaller tables: uses exact COUNT(*) (fast enough)
///
/// This avoids the slow COUNT(*) full table scan on large tables.
pub async fn get_table_count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let table_name = "transactions";

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

    if approx > 100_000 {
        Ok(approx)
    } else {
        // Exact count for small tables
        let exact: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
            .fetch_one(pool)
            .await?;
        Ok(exact.0)
    }
}
