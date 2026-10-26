//! `/slots` を組み立て済みのルーター（`server::router()`）で通す。コーチのトークンは infra のモック
//! （`MockTokenService`）で発行し、生徒のトークンは `POST /accounts` が返したものを使う。
//! 契約の形・絞り込み・ページングの細かい検査は interface の `tests/slots.rs`。

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

fn post_json(path: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut request = Request::post(path).header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    request.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn coach_publishes_and_student_finds_the_slot() {
    let router = server::router();
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role: Role::Coach,
        })
        .into_string();
    let slot = json!({
        "sport": "tennis",
        "starts_at": "2099-01-01T10:00:00Z",
        "ends_at": "2099-01-01T11:00:00Z",
    });

    let (status, published) = send(&router, post_json("/slots", Some(&coach), slot.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{published}");
    let (status, body) = send(&router, post_json("/slots", Some(&coach), slot.clone())).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "slot_overlaps");

    let (status, body) = send(
        &router,
        post_json(
            "/accounts",
            None,
            json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let student = body["access_token"].as_str().unwrap().to_string();

    // 生徒はコーチの操作を呼べない
    let (status, _) = send(&router, post_json("/slots", Some(&student), slot)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let request = Request::get("/slots?sport=tennis")
        .header(header::AUTHORIZATION, format!("Bearer {student}"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&router, request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["slot_id"], published["slot_id"]);
    assert_eq!(body["next_cursor"], Value::Null);
}
