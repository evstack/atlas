use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common;

const TX_HASH: &str = "0x3000000000000000000000000000000000000000000000000000000000000001";
const EMITTER: &str = "0x3000000000000000000000000000000000000001";

async fn seed_logs(pool: &sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
         ON CONFLICT (number) DO NOTHING",
    )
    .bind(3000i64)
    .bind(format!("0x{:064x}", 3000))
    .bind(format!("0x{:064x}", 2999))
    .bind(1_700_003_000i64)
    .bind(63_000i64)
    .bind(30_000_000i64)
    .bind(1i32)
    .execute(pool)
    .await
    .expect("seed block");

    sqlx::query(
        "INSERT INTO transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, timestamp)
         VALUES ($1, $2, 0, $3, $4, $5, $6, $7, $8, true, $9)
         ON CONFLICT (hash, block_number) DO NOTHING",
    )
    .bind(TX_HASH)
    .bind(3000i64)
    .bind("0x3000000000000000000000000000000000000002")
    .bind("0x3000000000000000000000000000000000000003")
    .bind(1_000_000_000_000_000_000i64)
    .bind(20_000_000_000i64)
    .bind(21_000i64)
    .bind(Vec::<u8>::new())
    .bind(1_700_003_000i64)
    .execute(pool)
    .await
    .expect("seed tx");

    sqlx::query(
        "INSERT INTO event_logs
            (tx_hash, log_index, address, topic0, topic1, data, block_number, decoded, decode_status, decoded_at, decode_attempted_at, decode_source)
         VALUES ($1, 0, $2, $3, $4, $5, $6, $7, 'decoded', NOW(), NOW(), 'direct_abi')
         ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING",
    )
    .bind(TX_HASH)
    .bind(EMITTER)
    .bind("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
    .bind("0x0000000000000000000000003000000000000000000000000000000000000002")
    .bind(vec![
        0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 9,
    ])
    .bind(3000i64)
    .bind(serde_json::json!({
        "event_name": "Transfer",
        "event_signature": "Transfer(address,uint256)",
        "decoded_params": [
            {
                "name": "from",
                "type": "address",
                "value": "0x3000000000000000000000000000000000000002",
                "indexed": true
            },
            {
                "name": "value",
                "type": "uint256",
                "value": "9",
                "indexed": false
            }
        ]
    }))
    .execute(pool)
    .await
    .expect("seed log");
}

async fn seed_pending_log(pool: &sqlx::PgPool, tx_hash: &str) {
    sqlx::query(
        "INSERT INTO transactions (hash, block_number, block_index, from_address, to_address, value, gas_price, gas_used, input_data, status, timestamp)
         VALUES ($1, $2, 1, $3, $4, $5, $6, $7, $8, true, $9)
         ON CONFLICT (hash, block_number) DO NOTHING",
    )
    .bind(tx_hash)
    .bind(3000i64)
    .bind("0x3000000000000000000000000000000000000004")
    .bind("0x3000000000000000000000000000000000000005")
    .bind(0i64)
    .bind(20_000_000_000i64)
    .bind(30_000i64)
    .bind(Vec::<u8>::new())
    .bind(1_700_003_001i64)
    .execute(pool)
    .await
    .expect("seed pending tx");

    sqlx::query(
        "INSERT INTO event_logs
            (tx_hash, log_index, address, topic0, data, block_number, decoded, decode_status)
         VALUES ($1, 0, $2, $3, $4, $5, NULL, 'pending')
         ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING",
    )
    .bind(tx_hash)
    .bind(EMITTER)
    .bind("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
    .bind(Vec::<u8>::new())
    .bind(3000i64)
    .execute(pool)
    .await
    .expect("seed pending log");
}

#[test]
fn transaction_logs_include_stored_decoded_fields() {
    common::run(async {
        let pool = common::pool();
        seed_logs(&pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/transactions/{TX_HASH}/logs"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let logs = body["data"].as_array().expect("logs array");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0]["event_name"].as_str(), Some("Transfer"));
        assert_eq!(logs[0]["decode_status"].as_str(), Some("decoded"));
        assert_eq!(logs[0]["decoded_params"][0]["name"].as_str(), Some("from"));
        assert_eq!(
            logs[0]["data"].as_str(),
            Some("0x0000000000000000000000000000000000000000000000000000000000000009")
        );
    });
}

#[test]
fn decoded_logs_endpoint_matches_stored_log_shape() {
    common::run(async {
        let pool = common::pool();
        seed_logs(&pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/transactions/{TX_HASH}/logs/decoded"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let logs = body["data"].as_array().expect("logs array");
        assert_eq!(
            logs[0]["event_signature"].as_str(),
            Some("Transfer(address,uint256)")
        );
        assert_eq!(logs[0]["decode_source"].as_str(), Some("direct_abi"));
    });
}

#[test]
fn decoded_logs_endpoint_falls_back_to_known_event_signatures() {
    common::run(async {
        let pool = common::pool();
        seed_logs(&pool).await;
        let pending_tx_hash = "0x3000000000000000000000000000000000000000000000000000000000000002";
        seed_pending_log(&pool, pending_tx_hash).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/transactions/{pending_tx_hash}/logs/decoded"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let logs = body["data"].as_array().expect("logs array");
        assert_eq!(logs[0]["event_name"].as_str(), Some("Transfer"));
        assert_eq!(
            logs[0]["event_signature"].as_str(),
            Some("Transfer(address,address,uint256)")
        );
        assert_eq!(logs[0]["decode_status"].as_str(), Some("pending"));
    });
}
