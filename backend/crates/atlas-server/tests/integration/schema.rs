use crate::common;

// ── Migration health ──────────────────────────────────────────────────────────

#[test]
fn migrations_are_idempotent() {
    // sqlx tracks applied migrations in _sqlx_migrations; re-running is a no-op.
    // This verifies the tracking table is intact and no migration errors on repeat.
    common::run(async {
        let pool = common::pool();
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations should be idempotent");
    });
}

// ── Table presence ────────────────────────────────────────────────────────────

#[test]
fn all_expected_tables_exist() {
    common::run(async {
        let pool = common::pool();
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename",
        )
        .fetch_all(&pool)
        .await
        .expect("query tables");

        for expected in [
            "addresses",
            "address_labels",
            "block_da_status",
            "blocks",
            "contract_abis",
            "erc20_balances",
            "erc20_contracts",
            "erc20_transfers",
            "event_logs",
            "event_signatures",
            "failed_blocks",
            "indexer_state",
            "nft_contracts",
            "nft_tokens",
            "nft_transfers",
            "proxy_contracts",
            "transactions",
            "tx_hash_lookup",
        ] {
            assert!(
                tables.contains(&expected.to_string()),
                "missing table: {expected}"
            );
        }
    });
}

#[test]
fn partitioned_tables_have_initial_partition() {
    common::run(async {
        let pool = common::pool();
        // Each partitioned table must have its _p0 partition created by the migration.
        let partitions: Vec<String> = sqlx::query_scalar(
            "SELECT relname::text FROM pg_class
             WHERE relname LIKE '%_p0' AND relkind = 'r'
             ORDER BY relname",
        )
        .fetch_all(&pool)
        .await
        .expect("query partitions");

        for expected in [
            "blocks_p0",
            "erc20_transfers_p0",
            "event_logs_p0",
            "nft_transfers_p0",
            "transactions_p0",
        ] {
            assert!(
                partitions.contains(&expected.to_string()),
                "missing partition: {expected}"
            );
        }
    });
}

// ── Index presence ────────────────────────────────────────────────────────────

#[test]
fn key_indexes_exist() {
    common::run(async {
        let pool = common::pool();
        // Include both regular indexes (relkind='i') and partitioned indexes (relkind='I').
        let indexes: Vec<String> = sqlx::query_scalar(
            "SELECT c.relname::text FROM pg_class c
             JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = 'public' AND c.relkind IN ('i', 'I')
             ORDER BY c.relname",
        )
        .fetch_all(&pool)
        .await
        .expect("query indexes");

        for expected in [
            // blocks
            "idx_blocks_hash",
            "idx_blocks_timestamp",
            // transactions
            "idx_transactions_block",
            "idx_transactions_from",
            "idx_transactions_to",
            // event_logs
            "idx_event_logs_address",
            "idx_event_logs_topic0",
            // addresses
            "idx_addresses_contract",
            // erc20
            "idx_erc20_balances_contract",
            // tx hash lookup (powers O(1) search)
            "tx_hash_lookup_pkey",
            // da status (powers pending-DA queries)
            "idx_block_da_status_pending",
        ] {
            assert!(
                indexes.contains(&expected.to_string()),
                "missing index: {expected}"
            );
        }
    });
}

#[test]
fn pg_trgm_extension_is_installed() {
    // Required for fuzzy search indexes on token names / symbols.
    common::run(async {
        let pool = common::pool();
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = 'pg_trgm')",
        )
        .fetch_one(&pool)
        .await
        .expect("query extension");

        assert!(exists, "pg_trgm extension must be installed");
    });
}

// ── Constraint enforcement ────────────────────────────────────────────────────

#[test]
fn duplicate_block_number_is_rejected() {
    common::run(async {
        let pool = common::pool();

        // Use a block number unlikely to collide with other test seeds.
        let block_number: i64 = 9_000_001;

        sqlx::query(
            "INSERT INTO blocks (number, hash, parent_hash, timestamp,
             gas_used, gas_limit, transaction_count)
             VALUES ($1, $2, $3, 0, 0, 0, 0)
             ON CONFLICT DO NOTHING",
        )
        .bind(block_number)
        .bind(format!("0x{:064x}", block_number))
        .bind(format!("0x{:064x}", block_number - 1))
        .execute(&pool)
        .await
        .expect("insert block");

        // Second insert with same number must fail (PK violation).
        let result = sqlx::query(
            "INSERT INTO blocks (number, hash, parent_hash, timestamp,
             gas_used, gas_limit, transaction_count)
             VALUES ($1, $2, $3, 0, 0, 0, 0)",
        )
        .bind(block_number)
        .bind(format!("0x{:064x}", block_number + 9999))
        .bind(format!("0x{:064x}", block_number - 1))
        .execute(&pool)
        .await;

        assert!(result.is_err(), "duplicate block number should be rejected");
    });
}

#[test]
fn duplicate_transaction_is_rejected() {
    common::run(async {
        let pool = common::pool();

        let block_number: i64 = 9_000_002;
        let tx_hash = format!("0x{:064x}", block_number);

        // Ensure the parent block exists.
        sqlx::query(
            "INSERT INTO blocks (number, hash, parent_hash, timestamp,
             gas_used, gas_limit, transaction_count)
             VALUES ($1, $2, $3, 0, 0, 0, 1)
             ON CONFLICT DO NOTHING",
        )
        .bind(block_number)
        .bind(format!("0x{:064x}", block_number + 1))
        .bind(format!("0x{:064x}", block_number - 1))
        .execute(&pool)
        .await
        .expect("insert parent block");

        sqlx::query(
            "INSERT INTO transactions
             (hash, block_number, block_index, from_address, value,
              gas_price, gas_used, input_data, status, timestamp)
             VALUES ($1, $2, 0, '0xdead', 0, 1, 21000, '\\x', true, 0)",
        )
        .bind(&tx_hash)
        .bind(block_number)
        .execute(&pool)
        .await
        .expect("insert tx");

        let result = sqlx::query(
            "INSERT INTO transactions
             (hash, block_number, block_index, from_address, value,
              gas_price, gas_used, input_data, status, timestamp)
             VALUES ($1, $2, 0, '0xdead', 0, 1, 21000, '\\x', true, 0)",
        )
        .bind(&tx_hash)
        .bind(block_number)
        .execute(&pool)
        .await;

        assert!(result.is_err(), "duplicate transaction should be rejected");
    });
}

#[test]
fn duplicate_erc20_transfer_is_rejected() {
    common::run(async {
        let pool = common::pool();

        let block_number: i64 = 9_000_003;
        let tx_hash = format!("0x{:064x}", block_number + 10000);

        let insert = || {
            sqlx::query(
                "INSERT INTO erc20_transfers
                 (tx_hash, log_index, block_number, contract_address,
                  from_address, to_address, value, timestamp)
                 VALUES ($1, 0, $2, '0xtoken', '0xfrom', '0xto', '100', 0)",
            )
            .bind(&tx_hash)
            .bind(block_number)
        };

        insert()
            .execute(&pool)
            .await
            .expect("insert erc20 transfer");
        let result = insert().execute(&pool).await;

        assert!(
            result.is_err(),
            "duplicate erc20 transfer should be rejected by unique constraint"
        );
    });
}
