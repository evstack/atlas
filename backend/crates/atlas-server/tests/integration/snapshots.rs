use crate::common;

/// Check if a command is available on PATH.
fn has_command(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
#[ignore] // Requires pg_dump and pg_restore on PATH
fn snapshot_dump_and_restore_round_trip() {
    if !has_command("pg_dump") || !has_command("pg_restore") {
        eprintln!("Skipping: pg_dump/pg_restore not found on PATH");
        return;
    }

    common::run(async {
        let pool = common::pool();
        let db_url = common::database_url();

        // Insert test data
        sqlx::query("INSERT INTO indexer_state (key, value) VALUES ('snapshot_test', 'hello') ON CONFLICT (key) DO UPDATE SET value = 'hello'")
            .execute(pool)
            .await
            .expect("insert test data");

        // pg_dump to temp file
        let dir = tempfile::tempdir().expect("create temp dir");
        let dump_path = dir.path().join("test_snapshot.dump");

        let dump_status = tokio::process::Command::new("pg_dump")
            .arg("--dbname")
            .arg(db_url)
            .arg("-Fc")
            .arg("-f")
            .arg(&dump_path)
            .status()
            .await
            .expect("spawn pg_dump");

        assert!(dump_status.success(), "pg_dump failed: {dump_status}");

        let metadata = tokio::fs::metadata(&dump_path).await.expect("stat dump file");
        assert!(metadata.len() > 0, "dump file is empty");

        // Create a separate database for restore
        sqlx::query("CREATE DATABASE test_restore")
            .execute(pool)
            .await
            .expect("create test_restore database");

        let restore_url = db_url.replace("/postgres", "/test_restore");

        // pg_restore into the new database
        let restore_status = tokio::process::Command::new("pg_restore")
            .arg("--dbname")
            .arg(&restore_url)
            .arg(&dump_path)
            .status()
            .await
            .expect("spawn pg_restore");

        assert!(restore_status.success(), "pg_restore failed: {restore_status}");

        // Verify data in restored database
        let restore_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&restore_url)
            .await
            .expect("connect to restored database");

        let row: (String,) =
            sqlx::query_as("SELECT value FROM indexer_state WHERE key = 'snapshot_test'")
                .fetch_one(&restore_pool)
                .await
                .expect("query restored data");

        assert_eq!(row.0, "hello");

        // Cleanup
        restore_pool.close().await;
        sqlx::query("DROP DATABASE test_restore")
            .execute(pool)
            .await
            .expect("drop test_restore database");
    });
}
