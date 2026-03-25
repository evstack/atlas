use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common;

// Block range: 6000-6999

const TOKEN_A: &str = "0x6000000000000000000000000000000000000001";
const TOKEN_B: &str = "0x6000000000000000000000000000000000000002";
const HOLDER_1: &str = "0x6000000000000000000000000000000000000010";
const HOLDER_2: &str = "0x6000000000000000000000000000000000000011";
const TX_HASH: &str = "0x6000000000000000000000000000000000000000000000000000000000000001";

async fn seed_token_data(pool: &sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
         ON CONFLICT (number) DO NOTHING",
    )
    .bind(6000i64)
    .bind(format!("0x{:064x}", 6000))
    .bind(format!("0x{:064x}", 5999))
    .bind(1_700_006_000i64)
    .bind(100_000i64)
    .bind(30_000_000i64)
    .bind(1i32)
    .execute(pool)
    .await
    .expect("seed block");

    sqlx::query(
        "INSERT INTO erc20_contracts (address, name, symbol, decimals, total_supply, first_seen_block)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (address) DO NOTHING",
    )
    .bind(TOKEN_A)
    .bind("Test Token A")
    .bind("TTA")
    .bind(18i16)
    .bind(bigdecimal::BigDecimal::from(1_000_000i64))
    .bind(6000i64)
    .execute(pool)
    .await
    .expect("seed erc20 contract A");

    sqlx::query(
        "INSERT INTO erc20_contracts (address, name, symbol, decimals, first_seen_block)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (address) DO NOTHING",
    )
    .bind(TOKEN_B)
    .bind("Test Token B")
    .bind("TTB")
    .bind(6i16)
    .bind(6001i64)
    .execute(pool)
    .await
    .expect("seed erc20 contract B");

    for (holder, balance) in [(HOLDER_1, 700_000i64), (HOLDER_2, 300_000i64)] {
        sqlx::query(
            "INSERT INTO erc20_balances (address, contract_address, balance, last_updated_block)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (address, contract_address) DO NOTHING",
        )
        .bind(holder)
        .bind(TOKEN_A)
        .bind(bigdecimal::BigDecimal::from(balance))
        .bind(6000i64)
        .execute(pool)
        .await
        .expect("seed balance");
    }

    sqlx::query(
        "INSERT INTO transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, timestamp)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (hash, block_number) DO NOTHING",
    )
    .bind(TX_HASH)
    .bind(6000i64)
    .bind(0i32)
    .bind(HOLDER_1)
    .bind(TOKEN_A)
    .bind(0i64)
    .bind(20_000_000_000i64)
    .bind(60_000i64)
    .bind(Vec::<u8>::new())
    .bind(true)
    .bind(1_700_006_000i64)
    .execute(pool)
    .await
    .expect("seed transaction");

    sqlx::query(
        "INSERT INTO erc20_transfers (tx_hash, log_index, contract_address, from_address, to_address, value, block_number, timestamp)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING",
    )
    .bind(TX_HASH)
    .bind(0i32)
    .bind(TOKEN_A)
    .bind(HOLDER_1)
    .bind(HOLDER_2)
    .bind(bigdecimal::BigDecimal::from(50_000i64))
    .bind(6000i64)
    .bind(1_700_006_000i64)
    .execute(pool)
    .await
    .expect("seed erc20 transfer");
}

async fn seed_token_chart_data(pool: &sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
         ON CONFLICT (number) DO NOTHING",
    )
    .bind(9001i64)
    .bind(format!("0x{:064x}", 9001))
    .bind(format!("0x{:064x}", 9000))
    .bind(4_100_100_123i64)
    .bind(100_000i64)
    .bind(30_000_000i64)
    .bind(1i32)
    .execute(pool)
    .await
    .expect("seed token chart block");

    sqlx::query(
        "INSERT INTO transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, timestamp)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (hash, block_number) DO NOTHING",
    )
    .bind("0x9001000000000000000000000000000000000000000000000000000000000000")
    .bind(9001i64)
    .bind(0i32)
    .bind(HOLDER_1)
    .bind(TOKEN_B)
    .bind(0i64)
    .bind(20_000_000_000i64)
    .bind(60_000i64)
    .bind(Vec::<u8>::new())
    .bind(true)
    .bind(4_100_100_123i64)
    .execute(pool)
    .await
    .expect("seed token chart transaction");

    sqlx::query(
        "INSERT INTO erc20_transfers (tx_hash, log_index, contract_address, from_address, to_address, value, block_number, timestamp)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING",
    )
    .bind("0x9001000000000000000000000000000000000000000000000000000000000000")
    .bind(0i32)
    .bind(TOKEN_B)
    .bind(HOLDER_1)
    .bind(HOLDER_2)
    .bind(bigdecimal::BigDecimal::from(75_000i64))
    .bind(9001i64)
    .bind(4_100_100_123i64)
    .execute(pool)
    .await
    .expect("seed token chart transfer");
}

#[test]
fn list_tokens() {
    common::run(async {
        let pool = common::pool();
        seed_token_data(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tokens?page=1&limit=100")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        assert!(body["data"].as_array().unwrap().len() >= 2);
    });
}

#[test]
fn get_token_detail() {
    common::run(async {
        let pool = common::pool();
        seed_token_data(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/tokens/{}", TOKEN_A))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        assert_eq!(body["address"].as_str().unwrap(), TOKEN_A);
        assert_eq!(body["name"].as_str().unwrap(), "Test Token A");
        assert_eq!(body["symbol"].as_str().unwrap(), "TTA");
        assert_eq!(body["holder_count"].as_i64().unwrap(), 2);
        assert_eq!(body["transfer_count"].as_i64().unwrap(), 1);
    });
}

#[test]
fn get_token_holders() {
    common::run(async {
        let pool = common::pool();
        seed_token_data(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/tokens/{}/holders", TOKEN_A))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);

        // First holder should have largest balance (sorted DESC)
        let first_pct = data[0]["percentage"].as_f64().unwrap();
        assert!(first_pct > 50.0); // 700k/1M = 70%
    });
}

#[test]
fn get_tx_erc20_transfers() {
    common::run(async {
        let pool = common::pool();
        seed_token_data(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/transactions/{}/erc20-transfers", TX_HASH))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["contract_address"].as_str().unwrap(), TOKEN_A);
        assert_eq!(data[0]["from_address"].as_str().unwrap(), HOLDER_1);
        assert_eq!(data[0]["to_address"].as_str().unwrap(), HOLDER_2);
    });
}

#[test]
fn get_token_chart_returns_exact_bucket_count_for_non_aligned_window() {
    common::run(async {
        let pool = common::pool();
        seed_token_data(pool).await;
        seed_token_chart_data(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/tokens/{}/chart?window=1h", TOKEN_B))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        assert_eq!(body.as_array().unwrap().len(), 12);
    });
}
