use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common;

// Block range: 5000-5999

const ADDR: &str = "0x5000000000000000000000000000000000000001";
const ADDR_TO: &str = "0x5000000000000000000000000000000000000002";
const TX_HASH_A: &str = "0x5000000000000000000000000000000000000000000000000000000000000001";
const TX_HASH_B: &str = "0x5000000000000000000000000000000000000000000000000000000000000002";

async fn seed_address_data(pool: &sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
         ON CONFLICT (number) DO NOTHING",
    )
    .bind(5000i64)
    .bind(format!("0x{:064x}", 5000))
    .bind(format!("0x{:064x}", 4999))
    .bind(1_700_005_000i64)
    .bind(42_000i64)
    .bind(30_000_000i64)
    .bind(2i32)
    .execute(pool)
    .await
    .expect("seed block");

    sqlx::query(
        "INSERT INTO addresses (address, is_contract, first_seen_block, tx_count)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (address) DO NOTHING",
    )
    .bind(ADDR)
    .bind(true)
    .bind(5000i64)
    .bind(2i32)
    .execute(pool)
    .await
    .expect("seed address");

    for (idx, hash) in [TX_HASH_A, TX_HASH_B].iter().enumerate() {
        sqlx::query(
            "INSERT INTO transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (hash, block_number) DO NOTHING",
        )
        .bind(hash)
        .bind(5000i64)
        .bind(idx as i32)
        .bind(ADDR)
        .bind(ADDR_TO)
        .bind(0i64)
        .bind(20_000_000_000i64)
        .bind(21_000i64)
        .bind(Vec::<u8>::new())
        .bind(true)
        .bind(1_700_005_000i64)
        .execute(pool)
        .await
        .expect("seed transaction");
    }
}

#[test]
fn get_address_detail() {
    common::run(async {
        let pool = common::pool();
        seed_address_data(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/addresses/{}", ADDR))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        assert_eq!(body["address"].as_str().unwrap(), ADDR);
        assert_eq!(body["address_type"].as_str().unwrap(), "contract");
        assert_eq!(body["tx_count"].as_i64().unwrap(), 2);
    });
}

#[test]
fn get_address_transactions() {
    common::run(async {
        let pool = common::pool();
        seed_address_data(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/addresses/{}/transactions", ADDR))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);
    });
}
