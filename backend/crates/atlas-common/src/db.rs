use sqlx::postgres::PgPoolOptions;
use sqlx::{Executor, PgPool};

const BLOCK_DA_STATUS_MIGRATION_VERSION: i64 = 20240108000001;
const BLOCK_DA_STATUS_CURRENT_CHECKSUM: &str =
    "39f2294fd8beb9085020cd0dfcbd5a7883a7b6eb25bcb02ee8480f7f419f564818c8fd4f543ef930bedb57aaaf242392";
const BLOCK_DA_STATUS_MAR19_CHECKSUM: &str =
    "79d676ef50b7fbafc58dc5dfc55d9efb8c30fc7887bcd8968155fa2df38b85394d23f7fa64c003438062249862a6aa00";

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
    reconcile_known_migration_checksums(&pool).await?;
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .map_err(|e| sqlx::Error::Migrate(Box::new(e)))?;
    Ok(())
}

async fn reconcile_known_migration_checksums(pool: &PgPool) -> Result<(), sqlx::Error> {
    let migrations_table_exists: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('_sqlx_migrations')::text")
            .fetch_one(pool)
            .await?;
    if migrations_table_exists.is_none() {
        return Ok(());
    }

    let stored_checksum: Option<String> = sqlx::query_scalar(
        "SELECT encode(checksum, 'hex') FROM _sqlx_migrations WHERE version = $1",
    )
    .bind(BLOCK_DA_STATUS_MIGRATION_VERSION)
    .fetch_optional(pool)
    .await?;

    let Some(expected_checksum) =
        replacement_checksum_for_known_mismatch(stored_checksum.as_deref())
    else {
        return Ok(());
    };

    // March 16 and March 19 shipped different checksums for this migration, but the SQL
    // only changed in a comment. Normalize the stored checksum so sqlx can continue.
    sqlx::query("UPDATE _sqlx_migrations SET checksum = decode($1, 'hex') WHERE version = $2")
        .bind(expected_checksum)
        .bind(BLOCK_DA_STATUS_MIGRATION_VERSION)
        .execute(pool)
        .await?;

    Ok(())
}

fn replacement_checksum_for_known_mismatch(stored_checksum: Option<&str>) -> Option<&'static str> {
    match stored_checksum {
        Some(BLOCK_DA_STATUS_CURRENT_CHECKSUM) | None => None,
        Some(BLOCK_DA_STATUS_MAR19_CHECKSUM) => Some(BLOCK_DA_STATUS_CURRENT_CHECKSUM),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        replacement_checksum_for_known_mismatch, BLOCK_DA_STATUS_CURRENT_CHECKSUM,
        BLOCK_DA_STATUS_MAR19_CHECKSUM,
    };

    #[test]
    fn known_current_checksum_needs_no_reconciliation() {
        assert_eq!(
            replacement_checksum_for_known_mismatch(Some(BLOCK_DA_STATUS_CURRENT_CHECKSUM)),
            None
        );
    }

    #[test]
    fn known_mar19_checksum_is_reconciled_to_current() {
        assert_eq!(
            replacement_checksum_for_known_mismatch(Some(BLOCK_DA_STATUS_MAR19_CHECKSUM)),
            Some(BLOCK_DA_STATUS_CURRENT_CHECKSUM)
        );
    }

    #[test]
    fn unknown_checksum_is_left_untouched() {
        assert_eq!(
            replacement_checksum_for_known_mismatch(Some("deadbeef")),
            None
        );
    }

    #[test]
    fn missing_checksum_is_ignored() {
        assert_eq!(replacement_checksum_for_known_mismatch(None), None);
    }
}
