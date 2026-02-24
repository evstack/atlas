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

/// Run database migrations
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../migrations").run(pool).await
}
