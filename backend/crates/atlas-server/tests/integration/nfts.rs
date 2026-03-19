use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common;

// Block range: 7000-7999

const NFT_A: &str = "0x7000000000000000000000000000000000000001";
const NFT_B: &str = "0x7000000000000000000000000000000000000002";
const OWNER: &str = "0x7000000000000000000000000000000000000010";
const TX_HASH_NFT: &str = "0x7000000000000000000000000000000000000000000000000000000000000001";

async fn seed_nft_data(pool: &sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count, indexed_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
         ON CONFLICT (number) DO NOTHING",
    )
    .bind(7000i64)
    .bind(format!("0x{:064x}", 7000))
    .bind(format!("0x{:064x}", 6999))
    .bind(1_700_007_000i64)
    .bind(100_000i64)
    .bind(30_000_000i64)
    .bind(1i32)
    .execute(pool)
    .await
    .expect("seed block");

    for (addr, name, symbol) in [(NFT_A, "Apes", "APE"), (NFT_B, "Punks", "PUNK")] {
        sqlx::query(
            "INSERT INTO nft_contracts (address, name, symbol, total_supply, first_seen_block)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (address) DO NOTHING",
        )
        .bind(addr)
        .bind(name)
        .bind(symbol)
        .bind(100i64)
        .bind(7000i64)
        .execute(pool)
        .await
        .expect("seed nft contract");
    }

    for token_id in 1..=3i64 {
        sqlx::query(
            "INSERT INTO nft_tokens (contract_address, token_id, owner, metadata_fetched, last_transfer_block)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (contract_address, token_id) DO NOTHING",
        )
        .bind(NFT_A)
        .bind(bigdecimal::BigDecimal::from(token_id))
        .bind(OWNER)
        .bind(false)
        .bind(7000i64)
        .execute(pool)
        .await
        .expect("seed nft token");
    }

    for (log_idx, token_id) in [0i32, 1, 2].iter().zip(1..=3i64) {
        sqlx::query(
            "INSERT INTO nft_transfers (tx_hash, log_index, contract_address, token_id, from_address, to_address, block_number, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (tx_hash, log_index, block_number) DO NOTHING",
        )
        .bind(TX_HASH_NFT)
        .bind(log_idx)
        .bind(NFT_A)
        .bind(bigdecimal::BigDecimal::from(token_id))
        .bind("0x0000000000000000000000000000000000000000")
        .bind(OWNER)
        .bind(7000i64)
        .bind(1_700_007_000i64)
        .execute(pool)
        .await
        .expect("seed nft transfer");
    }
}

#[test]
fn list_nft_collections() {
    common::run(async {
        let pool = common::pool();
        seed_nft_data(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/nfts/collections?page=1&limit=100")
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
fn list_collection_tokens() {
    common::run(async {
        let pool = common::pool();
        seed_nft_data(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/nfts/collections/{}/tokens", NFT_A))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 3);

        // Ordered by token_id ASC (token_id is BigDecimal, may serialize as string or number)
        let parse_token_id = |v: &serde_json::Value| -> i64 {
            v.as_i64()
                .unwrap_or_else(|| v.as_str().unwrap().parse().unwrap())
        };
        let id0 = parse_token_id(&data[0]["token_id"]);
        let id1 = parse_token_id(&data[1]["token_id"]);
        assert!(id0 < id1);
    });
}

#[test]
fn get_collection_transfers() {
    common::run(async {
        let pool = common::pool();
        seed_nft_data(pool).await;

        let app = common::test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/nfts/collections/{}/transfers", NFT_A))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = common::json_body(response).await;
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 3);
        assert_eq!(body["total"].as_i64().unwrap(), 3);
    });
}
