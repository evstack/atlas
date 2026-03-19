use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common;

// Block range: 1000-1999

async fn seed_blocks(pool: &sqlx::PgPool) {
    for i in 1000..1005 {
        sqlx::query(
            "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
             ON CONFLICT (number) DO NOTHING",
        )
        .bind(i as i64)
        .bind(format!("0x{:064x}", i))
        .bind(format!("0x{:064x}", i - 1))
        .bind(1_700_000_000i64 + i as i64)
        .bind(21_000i64)
        .bind(30_000_000i64)
        .bind(0i32)
        .execute(pool)
        .await
        .expect("seed block");
    }
}

#[test]
fn list_blocks_paginated() {
    common::run(async {
        let pool = common::pool();
        seed_blocks(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/blocks?page=1&limit=2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        assert_eq!(body["data"].as_array().unwrap().len(), 2);
        assert!(body["total"].as_i64().unwrap() >= 5);

        // Blocks should be in DESC order
        let first = body["data"][0]["number"].as_i64().unwrap();
        let second = body["data"][1]["number"].as_i64().unwrap();
        assert!(first > second);
    });
}

#[test]
fn get_block_by_number() {
    common::run(async {
        let pool = common::pool();
        seed_blocks(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/blocks/1002")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        assert_eq!(body["number"].as_i64().unwrap(), 1002);
        assert_eq!(body["hash"].as_str().unwrap(), &format!("0x{:064x}", 1002));
        assert_eq!(body["gas_used"].as_i64().unwrap(), 21_000);
    });
}

#[test]
fn get_block_not_found() {
    common::run(async {
        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/blocks/999999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    });
}
