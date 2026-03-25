use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common;

async fn seed_chart_data(pool: &sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
         ON CONFLICT (number) DO NOTHING",
    )
    .bind(9000i64)
    .bind(format!("0x{:064x}", 9000))
    .bind(format!("0x{:064x}", 8999))
    .bind(4_100_000_123i64)
    .bind(100_000i64)
    .bind(30_000_000i64)
    .bind(1i32)
    .execute(pool)
    .await
    .expect("seed chart block");

    sqlx::query(
        "INSERT INTO transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, timestamp)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (hash, block_number) DO NOTHING",
    )
    .bind("0x9000000000000000000000000000000000000000000000000000000000000000")
    .bind(9000i64)
    .bind(0i32)
    .bind("0x9000000000000000000000000000000000000001")
    .bind("0x9000000000000000000000000000000000000002")
    .bind(0i64)
    .bind(20_000_000_000i64)
    .bind(21_000i64)
    .bind(Vec::<u8>::new())
    .bind(true)
    .bind(4_100_000_123i64)
    .execute(pool)
    .await
    .expect("seed chart transaction");
}

#[test]
fn health_returns_ok() {
    common::run(async {
        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    });
}

#[test]
fn status_returns_chain_info() {
    common::run(async {
        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        assert_eq!(body["chain_id"].as_str().unwrap(), "42");
        assert_eq!(body["chain_name"].as_str().unwrap(), "Test Chain");
        assert!(body["block_height"].is_i64());
    });
}

#[test]
fn stats_charts_return_exact_bucket_count_for_non_aligned_window() {
    common::run(async {
        let pool = common::pool();
        seed_chart_data(pool).await;

        let app = common::test_router();

        let blocks_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/stats/blocks-chart?window=1h")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(blocks_response.status(), StatusCode::OK);
        let blocks_body = common::json_body(blocks_response).await;
        assert_eq!(blocks_body.as_array().unwrap().len(), 12);

        let gas_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/stats/gas-price?window=1h")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(gas_response.status(), StatusCode::OK);
        let gas_body = common::json_body(gas_response).await;
        assert_eq!(gas_body.as_array().unwrap().len(), 12);
    });
}
