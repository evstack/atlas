use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common;

// Block range: 2000-2999

const TX_HASH_1: &str = "0x2000000000000000000000000000000000000000000000000000000000000001";
const TX_HASH_2: &str = "0x2000000000000000000000000000000000000000000000000000000000000002";
const TX_HASH_3: &str = "0x2000000000000000000000000000000000000000000000000000000000000003";
const FROM_ADDR: &str = "0x2000000000000000000000000000000000000001";
const TO_ADDR: &str = "0x2000000000000000000000000000000000000002";

async fn seed_transactions(pool: &sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
         ON CONFLICT (number) DO NOTHING",
    )
    .bind(2000i64)
    .bind(format!("0x{:064x}", 2000))
    .bind(format!("0x{:064x}", 1999))
    .bind(1_700_002_000i64)
    .bind(63_000i64)
    .bind(30_000_000i64)
    .bind(3i32)
    .execute(pool)
    .await
    .expect("seed block");

    let hashes = [TX_HASH_1, TX_HASH_2, TX_HASH_3];
    for (idx, hash) in hashes.iter().enumerate() {
        sqlx::query(
            "INSERT INTO transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (hash, block_number) DO NOTHING",
        )
        .bind(hash)
        .bind(2000i64)
        .bind(idx as i32)
        .bind(FROM_ADDR)
        .bind(TO_ADDR)
        .bind(1_000_000_000_000_000_000i64)
        .bind(20_000_000_000i64)
        .bind(21_000i64)
        .bind(Vec::<u8>::new())
        .bind(true)
        .bind(1_700_002_000i64)
        .execute(pool)
        .await
        .expect("seed transaction");
    }
}

#[test]
fn list_transactions() {
    common::run(async {
        let pool = common::pool();
        seed_transactions(&pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/transactions?page=1&limit=100")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let data = body["data"].as_array().unwrap();
        assert!(data.len() >= 3);
    });
}

#[test]
fn get_transaction_by_hash() {
    common::run(async {
        let pool = common::pool();
        seed_transactions(&pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/transactions/{}", TX_HASH_1))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        assert_eq!(body["hash"].as_str().unwrap(), TX_HASH_1);
        assert_eq!(body["block_number"].as_i64().unwrap(), 2000);
        assert!(body["status"].as_bool().unwrap());
    });
}

#[test]
fn get_transaction_not_found() {
    common::run(async {
        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/transactions/0x0000000000000000000000000000000000000000000000000000000000000000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    });
}

#[test]
fn get_block_transactions() {
    common::run(async {
        let pool = common::pool();
        seed_transactions(&pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/blocks/2000/transactions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 3);

        // Should be ordered by block_index ASC
        let idx0 = data[0]["block_index"].as_i64().unwrap();
        let idx1 = data[1]["block_index"].as_i64().unwrap();
        let idx2 = data[2]["block_index"].as_i64().unwrap();
        assert!(idx0 < idx1 && idx1 < idx2);
    });
}
