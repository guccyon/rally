//! `GET /entitlements/me` を組み立て済みのルーター（`server::router()`）で通す。受講権を作る月額契約の開始は T11 なので、
//! `POST /accounts` で登録したばかりの生徒は 404、コーチは 403。契約の形・月をまたいだ枠の検査は interface の
//! `tests/entitlements.rs`。

use application::identity::{TokenClaims, TokenIssuer};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role};
use infra::MockTokenService;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn get_my_entitlement(token: &str) -> Request<Body> {
    Request::get("/entitlements/me")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn registered_student_has_no_entitlement_yet() {
    let router = server::router();
    let (status, account) = send(
        &router,
        Request::post("/accounts")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })
                    .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");

    let (status, body) = send(
        &router,
        get_my_entitlement(account["access_token"].as_str().unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "not_found");
}

#[tokio::test]
async fn coach_is_forbidden() {
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role: Role::Coach,
        })
        .into_string();
    let (status, body) = send(&server::router(), get_my_entitlement(&coach)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}
