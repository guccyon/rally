//! `/bookings` を組み立て済みのルーター（`server::router()`）で通す。コーチが枠を公開し、`POST /accounts` で
//! 登録した生徒が体験予約して確定するまで（一周シナリオの入口）と、確定した予約のキャンセル。境界間イベントは
//! infra のモック（標準出力に書くだけ）。契約の形・拒否の細かい検査は interface の `tests/bookings.rs`。

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

async fn register(router: &Router, email: &str) -> String {
    let (status, body) = send(
        router,
        post_json(
            "/accounts",
            None,
            json!({ "email": email, "display_name": "たろう", "sport": "tennis" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn student_books_a_trial_and_confirms_it() {
    let router = server::router();
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role: Role::Coach,
        })
        .into_string();
    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            Some(&coach),
            json!({
                "sport": "tennis",
                "starts_at": "2099-01-01T10:00:00Z",
                "ends_at": "2099-01-01T11:00:00Z",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let student = register(&router, "taro@example.com").await;
    let other = register(&router, "hanako@example.com").await;
    let request = json!({ "slot_id": slot["slot_id"], "is_trial": true });

    // コーチは予約できない
    let (status, _) = send(
        &router,
        post_json("/bookings", Some(&coach), request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, booking) = send(
        &router,
        post_json("/bookings", Some(&student), request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    assert_eq!(booking["status"], "pending");

    // 同じ枠は別の生徒も予約できない
    let (status, body) = send(&router, post_json("/bookings", Some(&other), request)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "slot_already_booked");

    let path = format!(
        "/bookings/{}/confirm",
        booking["booking_id"].as_str().unwrap()
    );
    let (status, body) = send(&router, post_json(&path, Some(&other), json!({}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, confirmed) = send(&router, post_json(&path, Some(&student), json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{confirmed}");
    assert_eq!(confirmed["status"], "confirmed");
    assert_eq!(confirmed["booking_id"], booking["booking_id"]);

    // 確定した枠は検索に出ない
    let request = Request::get("/slots?sport=tennis")
        .header(header::AUTHORIZATION, format!("Bearer {student}"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&router, request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"], json!([]));
}

#[tokio::test]
async fn student_cancels_a_confirmed_booking_and_the_slot_is_searchable_again() {
    let router = server::router();
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role: Role::Coach,
        })
        .into_string();
    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            Some(&coach),
            json!({
                "sport": "tennis",
                "starts_at": "2099-01-01T10:00:00Z",
                "ends_at": "2099-01-01T11:00:00Z",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let student = register(&router, "taro@example.com").await;
    let (status, booking) = send(
        &router,
        post_json(
            "/bookings",
            Some(&student),
            json!({ "slot_id": slot["slot_id"], "is_trial": false }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    let booking_id = booking["booking_id"].as_str().unwrap();
    let (status, _) = send(
        &router,
        post_json(
            &format!("/bookings/{booking_id}/confirm"),
            Some(&student),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let cancel = format!("/bookings/{booking_id}/cancel");
    let (status, _) = send(&router, post_json(&cancel, Some(&coach), json!({}))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "コーチはキャンセルできない");
    let (status, cancelled) = send(
        &router,
        post_json(
            &cancel,
            Some(&student),
            json!({ "reason": "都合が悪くなった" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["status"], "cancelled");
    assert!(cancelled["cancelled_at"].is_string());

    // キャンセルで空いた枠は検索に戻る
    let request = Request::get("/slots?sport=tennis")
        .header(header::AUTHORIZATION, format!("Bearer {student}"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&router, request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"][0]["slot_id"], slot["slot_id"]);
}
