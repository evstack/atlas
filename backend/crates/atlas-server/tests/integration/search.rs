use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common;

// Block range: 3000-3999

const SEARCH_BLOCK: i64 = 3000;
const SEARCH_TX_HASH: &str = "0x3000000000000000000000000000000000000000000000000000000000000001";
const SEARCH_ADDR: &str = "0x3000000000000000000000000000000000000001";

async fn seed_search_data(pool: &sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
         ON CONFLICT (number) DO NOTHING",
    )
    .bind(SEARCH_BLOCK)
    .bind(format!("0x{:064x}", SEARCH_BLOCK))
    .bind(format!("0x{:064x}", SEARCH_BLOCK - 1))
    .bind(1_700_003_000i64)
    .bind(21_000i64)
    .bind(30_000_000i64)
    .bind(1i32)
    .execute(pool)
    .await
    .expect("seed block");

    sqlx::query(
        "INSERT INTO transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, timestamp)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (hash, block_number) DO NOTHING",
    )
    .bind(SEARCH_TX_HASH)
    .bind(SEARCH_BLOCK)
    .bind(0i32)
    .bind(SEARCH_ADDR)
    .bind("0x3000000000000000000000000000000000000002")
    .bind(0i64)
    .bind(20_000_000_000i64)
    .bind(21_000i64)
    .bind(Vec::<u8>::new())
    .bind(true)
    .bind(1_700_003_000i64)
    .execute(pool)
    .await
    .expect("seed transaction");

    sqlx::query(
        "INSERT INTO tx_hash_lookup (hash, block_number)
         VALUES ($1, $2)
         ON CONFLICT (hash) DO NOTHING",
    )
    .bind(SEARCH_TX_HASH)
    .bind(SEARCH_BLOCK)
    .execute(pool)
    .await
    .expect("seed tx_hash_lookup");

    sqlx::query(
        "INSERT INTO addresses (address, is_contract, first_seen_block, tx_count)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (address) DO NOTHING",
    )
    .bind(SEARCH_ADDR)
    .bind(false)
    .bind(SEARCH_BLOCK)
    .bind(1i32)
    .execute(pool)
    .await
    .expect("seed address");
}

#[test]
fn search_by_block_hash() {
    common::run(async {
        let pool = common::pool();
        seed_search_data(&pool).await;

        // Search by block hash (66 chars = 0x + 64 hex)
        let block_hash = format!("0x{:064x}", SEARCH_BLOCK);
        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/search?q={}", block_hash))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let results = body["results"].as_array().unwrap();
        let block_result = results
            .iter()
            .find(|r| r["type"].as_str().unwrap() == "block");
        assert!(block_result.is_some());
        assert_eq!(
            block_result.unwrap()["number"].as_i64().unwrap(),
            SEARCH_BLOCK
        );
    });
}

#[test]
fn search_by_tx_hash() {
    common::run(async {
        let pool = common::pool();
        seed_search_data(&pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/search?q={}", SEARCH_TX_HASH))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let results = body["results"].as_array().unwrap();
        assert!(!results.is_empty());

        let tx_result = results
            .iter()
            .find(|r| r["type"].as_str().unwrap() == "transaction");
        assert!(tx_result.is_some());
        assert_eq!(tx_result.unwrap()["hash"].as_str().unwrap(), SEARCH_TX_HASH);
    });
}

#[test]
fn search_by_address() {
    common::run(async {
        let pool = common::pool();
        seed_search_data(&pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/search?q={}", SEARCH_ADDR))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let results = body["results"].as_array().unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0]["type"].as_str().unwrap(), "address");
        assert_eq!(results[0]["address"].as_str().unwrap(), SEARCH_ADDR);
    });
}
