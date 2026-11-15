//! `GET /version` の統合テスト。版は環境変数 `APP_VERSION`（[`server::APP_VERSION_VAR`]）で渡し、認証なしで返す。

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use serde_json::{Value, json};
use tower::ServiceExt;

fn router_with(app_version: Option<&str>) -> Router {
    server::with_app_version(server::router(), |name| {
        assert_eq!(name, "APP_VERSION");
        app_version.map(str::to_string)
    })
}

async fn get_version(router: Router, authorization: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/version");
    if let Some(authorization) = authorization {
        request = request.header(header::AUTHORIZATION, authorization);
    }
    let response = router
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn returns_app_version_without_authentication() {
    let (status, body) = get_version(router_with(Some("0.3.1")), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "app_version": "0.3.1" }));
}

#[tokio::test]
async fn returns_unknown_without_app_version() {
    let (status, body) = get_version(router_with(None), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "app_version": "unknown" }));
}

#[tokio::test]
async fn returns_unknown_for_blank_app_version() {
    let (_, body) = get_version(router_with(Some("  ")), None).await;

    assert_eq!(body, json!({ "app_version": "unknown" }));
}

#[tokio::test]
async fn ignores_an_invalid_token() {
    let (status, body) = get_version(router_with(Some("0.3.1")), Some("Bearer nope")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "app_version": "0.3.1" }));
}

#[tokio::test]
async fn other_routes_still_answer() {
    let response = router_with(Some("0.3.1"))
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
