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
    // Sum approximate reltuples across partitions if any, else use parent.
    // This is instant and reasonably accurate for large tables.
    let approx_partitions: (Option<f64>,) = sqlx::query_as(
        r#"
        SELECT SUM(c.reltuples) AS approx
        FROM pg_class c
        JOIN pg_inherits i ON i.inhrelid = c.oid
        JOIN pg_class p ON p.oid = i.inhparent
        WHERE p.relname = $1
        "#
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    let approx = if let Some(sum) = approx_partitions.0 {
        sum as i64
    } else {
        let parent: (Option<f64>,) = sqlx::query_as(
            "SELECT reltuples FROM pg_class WHERE relname = $1"
        )
        .bind(table_name)
        .fetch_one(pool)
        .await?;
        parent.0.unwrap_or(0.0) as i64
    };

    if approx > 100_000 {
        Ok(approx)
    } else {
        // Exact count for small tables
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
