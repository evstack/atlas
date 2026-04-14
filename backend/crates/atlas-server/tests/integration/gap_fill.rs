use std::sync::{LazyLock, Mutex};
use tokio::sync::broadcast;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use atlas_server::indexer::GapFillWorker;
use atlas_server::metrics::{install_prometheus_recorder, Metrics};

use super::common;

/// Serializes gap-fill tests: process_batch queries all eligible failed_blocks,
/// so tests must not run concurrently or they'll pick up each other's rows.
static SERIALIZER: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Minimal valid JSON-RPC batch response for a block with no transactions.
/// IDs match what fetch_blocks_batch sends: block=i*2, receipts=i*2+1 (i=0).
fn empty_block_response(block_number: u64) -> serde_json::Value {
    serde_json::json!([
        {
            "jsonrpc": "2.0",
            "id": 0,
            "result": {
                "hash": format!("0x{:064x}", block_number),
                "parentHash": format!("0x{:064x}", block_number.saturating_sub(1)),
                "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
                "miner": "0x0000000000000000000000000000000000000000",
                "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                "difficulty": "0x0",
                "number": format!("0x{:x}", block_number),
                "gasLimit": "0x1c9c380",
                "gasUsed": "0x0",
                "timestamp": "0x6123456",
                "extraData": "0x",
                "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "nonce": "0x0000000000000000",
                "baseFeePerGas": "0x1",
                "transactions": [],
                "uncles": [],
                "size": "0x1f4"
            }
        },
        {
            "jsonrpc": "2.0",
            "id": 1,
            "result": []
        }
    ])
}

/// JSON-RPC batch response with a block-level error.
fn rpc_error_response() -> serde_json::Value {
    serde_json::json!([
        {
            "jsonrpc": "2.0",
            "id": 0,
            "error": { "code": -32000, "message": "block not found" }
        },
        {
            "jsonrpc": "2.0",
            "id": 1,
            "result": []
        }
    ])
}

async fn reset_failed_blocks(pool: &sqlx::PgPool, block_number: u64) {
    // Clear ALL failed_blocks so process_batch doesn't pick up rows from other tests.
    sqlx::query("DELETE FROM failed_blocks")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM blocks WHERE number = $1")
        .bind(block_number as i64)
        .execute(pool)
        .await
        .ok();
}

fn make_worker(database_url: &str, rpc_url: &str) -> GapFillWorker {
    make_worker_with_metrics(database_url, rpc_url, Metrics::new())
}

fn make_worker_with_metrics(database_url: &str, rpc_url: &str, metrics: Metrics) -> GapFillWorker {
    let pool = common::pool();
    let (tx, _) = broadcast::channel(16);
    GapFillWorker::new(pool, database_url, rpc_url, 10, tx, metrics)
        .expect("worker construction should succeed")
}

fn read_gauge(body: &str, name: &str) -> Option<f64> {
    body.lines()
        .find_map(|line| line.strip_prefix(&format!("{name} ")))
        .and_then(|value| value.parse::<f64>().ok())
}

// ---------------------------------------------------------------------------
// Test 1: success path
// ---------------------------------------------------------------------------

#[test]
fn gap_fill_retries_failed_block() {
    const BLOCK: u64 = 990_001;
    let _guard = SERIALIZER.lock().unwrap();

    common::run(async {
        let pool = common::pool();
        let database_url = common::database_url();
        reset_failed_blocks(&pool, BLOCK).await;

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(empty_block_response(BLOCK)))
            .expect(1)
            .mount(&mock_server)
            .await;

        sqlx::query(
            "INSERT INTO failed_blocks (block_number, error_message, retry_count, last_failed_at)
             VALUES ($1, 'test error', 0, NOW() - INTERVAL '1 hour')",
        )
        .bind(BLOCK as i64)
        .execute(&pool)
        .await
        .expect("insert test row");

        let worker = make_worker(database_url, &mock_server.uri());
        let recovered = worker.process_batch().await.expect("process_batch");

        assert_eq!(recovered, 1, "expected 1 block recovered");

        let (remaining,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM failed_blocks WHERE block_number = $1")
                .bind(BLOCK as i64)
                .fetch_one(&pool)
                .await
                .expect("count failed_blocks");
        assert_eq!(remaining, 0, "block should be removed from failed_blocks");

        let (in_blocks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM blocks WHERE number = $1")
            .bind(BLOCK as i64)
            .fetch_one(&pool)
            .await
            .expect("count blocks");
        assert_eq!(in_blocks, 1, "block should be present in blocks table");
    });
}

#[test]
fn gap_fill_updates_missing_blocks_metric_after_recovery() {
    const BLOCK: u64 = 990_004;
    let _guard = SERIALIZER.lock().unwrap();

    common::run(async {
        let pool = common::pool();
        let database_url = common::database_url();
        reset_failed_blocks(&pool, BLOCK).await;

        let handle = install_prometheus_recorder();
        let metrics = Metrics::new();
        metrics.set_indexer_missing_blocks(1);

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(empty_block_response(BLOCK)))
            .expect(1)
            .mount(&mock_server)
            .await;

        sqlx::query(
            "INSERT INTO failed_blocks (block_number, error_message, retry_count, last_failed_at)
             VALUES ($1, 'test error', 0, NOW() - INTERVAL '1 hour')",
        )
        .bind(BLOCK as i64)
        .execute(&pool)
        .await
        .expect("insert test row");

        let worker = make_worker_with_metrics(database_url, &mock_server.uri(), metrics);
        let recovered = worker.process_batch().await.expect("process_batch");

        assert_eq!(recovered, 1, "expected 1 block recovered");

        let gauge = read_gauge(&handle.render(), "atlas_indexer_missing_blocks")
            .expect("missing blocks gauge should be exported");
        assert_eq!(
            gauge, 0.0,
            "missing blocks gauge should be refreshed after recovery"
        );
    });
}

// ---------------------------------------------------------------------------
// Test 2: failure path — RPC returns a block-level error
// ---------------------------------------------------------------------------

#[test]
fn gap_fill_increments_retry_count_on_rpc_error() {
    const BLOCK: u64 = 990_002;
    let _guard = SERIALIZER.lock().unwrap();

    common::run(async {
        let pool = common::pool();
        let database_url = common::database_url();
        reset_failed_blocks(&pool, BLOCK).await;

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(rpc_error_response()))
            .expect(1)
            .mount(&mock_server)
            .await;

        sqlx::query(
            "INSERT INTO failed_blocks (block_number, error_message, retry_count, last_failed_at)
             VALUES ($1, 'test error', 3, NOW() - INTERVAL '2 hours')",
        )
        .bind(BLOCK as i64)
        .execute(&pool)
        .await
        .expect("insert test row");

        let worker = make_worker(database_url, &mock_server.uri());
        let recovered = worker.process_batch().await.expect("process_batch");

        assert_eq!(recovered, 0, "no block should be recovered on RPC error");

        let (retry_count,): (i32,) =
            sqlx::query_as("SELECT retry_count FROM failed_blocks WHERE block_number = $1")
                .bind(BLOCK as i64)
                .fetch_one(&pool)
                .await
                .expect("fetch retry_count");
        assert_eq!(retry_count, 4, "retry_count should be incremented");
    });
}

// ---------------------------------------------------------------------------
// Test 3: backoff — recently failed block is not fetched
// ---------------------------------------------------------------------------

#[test]
fn gap_fill_skips_recently_failed_block() {
    const BLOCK: u64 = 990_003;
    let _guard = SERIALIZER.lock().unwrap();

    common::run(async {
        let pool = common::pool();
        let database_url = common::database_url();
        reset_failed_blocks(&pool, BLOCK).await;

        let mock_server = MockServer::start().await;
        // expect(0): any request received is a test failure
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&mock_server)
            .await;

        // last_failed_at = NOW() → within every backoff window
        sqlx::query(
            "INSERT INTO failed_blocks (block_number, error_message, retry_count, last_failed_at)
             VALUES ($1, 'test error', 3, NOW())",
        )
        .bind(BLOCK as i64)
        .execute(&pool)
        .await
        .expect("insert test row");

        let worker = make_worker(database_url, &mock_server.uri());
        let recovered = worker.process_batch().await.expect("process_batch");

        assert_eq!(
            recovered, 0,
            "no block should be processed within backoff window"
        );
        // mock_server Drop verifies expect(0) was satisfied
    });
}
