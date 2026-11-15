//! 通話セッションの開始・終了を、組み立て済みのルーター（`server::router_at`）で一周させる。通話基盤は infra の
//! モック（`MockVideoCallProvider`）で、予約は scheduling の保存先から読む。
//!
//! - 確定前の予約では開始できず（409）、確定すれば開始できる
//! - 終了するとセッションが `ended` になり、通話の長さが記録される

use std::time::{Duration, SystemTime};

use application::{
    identity::{TokenClaims, TokenIssuer},
    in_memory::ManualClock,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role};
use infra::MockTokenService;
use serde_json::{Value, json};
use tower::ServiceExt;

/// 2026-11-01T10:00:00Z（スロットの開始）
fn nov1_10() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200 + 10 * 3600)
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn post_json(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn a_confirmed_booking_can_be_started_and_ended() {
    let clock = ManualClock::new(nov1_10() - Duration::from_secs(86_400));
    let router = server::router_at(clock.clone());
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role: Role::Coach,
        })
        .into_string();
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
    let student = account["access_token"].as_str().unwrap().to_string();

    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            &coach,
            json!({
                "sport": "tennis",
                "starts_at": "2026-11-01T10:00:00Z",
                "ends_at": "2026-11-01T11:00:00Z"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let (status, booking) = send(
        &router,
        post_json(
            "/bookings",
            &student,
            json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    let booking_id = booking["booking_id"].as_str().unwrap();
    let start = json!({ "booking_id": booking_id });

    // 確定前の予約では開始できない
    let (status, body) = send(
        &router,
        post_json("/live-lesson-sessions", &student, start.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "invalid_booking_state");

    let (status, body) = send(
        &router,
        post_json(
            &format!("/bookings/{booking_id}/confirm"),
            &student,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // コーチが開始する
    clock.set(nov1_10());
    let (status, session) = send(
        &router,
        post_json("/live-lesson-sessions", &coach, start.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    assert_eq!(session["status"], "in_progress");
    assert!(
        session["room_token"]
            .as_str()
            .is_some_and(|token| token.starts_with("mock-room.")),
        "{session}"
    );
    let session_id = session["session_id"].as_str().unwrap();

    // 生徒が 45 分後に終了する
    clock.set(nov1_10() + Duration::from_secs(45 * 60));
    let (status, ended) = send(
        &router,
        post_json(
            &format!("/live-lesson-sessions/{session_id}/end"),
            &student,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ended}");
    assert_eq!(ended["status"], "ended");
    assert_eq!(ended["duration_sec"], 45 * 60);
    assert_eq!(ended["room_token"], Value::Null);
}
