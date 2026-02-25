use sqlx::postgres::PgPoolOptions;
use sqlx::{Executor, PgPool};

/// Create a database connection pool.
/// Sets statement_timeout = 10s on every connection to prevent slow queries
/// from exhausting the pool.
pub async fn create_pool(database_url: &str, max_connections: u32) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                conn.execute("SET statement_timeout = '10s'").await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await
}

/// Run database migrations using a dedicated connection without statement_timeout,
/// since migrations (index builds, bulk inserts) can legitimately exceed 10s.
pub async fn run_migrations(database_url: &str) -> Result<(), sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .map_err(|e| sqlx::Error::Migrate(Box::new(e)))?;
    Ok(())
}
