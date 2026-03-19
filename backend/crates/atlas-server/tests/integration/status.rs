use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common;

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
