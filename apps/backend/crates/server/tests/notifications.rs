//! 予約の確定でのプッシュ通知を、組み立て済みのルーターで確かめる（API の E2E）。通知は notification の購読
//! （`PushNotificationSubscriber`）が境界間イベント `BookingConfirmed` で送り、scheduling は notification を直接呼ばない。
//!
//! 1. 生徒の登録 → コーチが枠を公開 → 生徒が予約（作成だけでは送らない）
//! 2. 予約の確定 → 生徒に `BookingConfirmed` の通知が 1 件届く（送り先はモック `MockPushSender`）
//! 3. 予約のキャンセルでは送らない（キャンセルの通知はまだない）

use std::time::{Duration, SystemTime};

use application::{
    identity::{TokenClaims, TokenIssuer, account_id_uuid},
    in_memory::{ManualClock, RecordingAnalyticsSink},
    scheduling::booking_id_uuid,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    identity::{AccountId, Role},
    notification::PushNotification,
};
use infra::{MockPushSender, MockTokenService};
use serde_json::{Value, json};
use tower::ServiceExt;

/// 2026-10-05T00:00:00Z（シナリオの始まり）
fn oct5() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_158_400)
}

async fn call(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
    expected: StatusCode,
) -> Value {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = request.body(Body::from(body.to_string())).unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    assert_eq!(status, expected, "{method} {path}: {body}");
    body
}

#[tokio::test]
async fn confirming_a_booking_pushes_a_notification_to_the_student() {
    let push = MockPushSender::new();
    let router = server::router_with_push_sender(
        ManualClock::new(oct5()),
        RecordingAnalyticsSink::default(),
        push.clone(),
    );
    let coach_id = AccountId::from_u128(42);
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: coach_id,
            role: Role::Coach,
        })
        .into_string();

    // 1. 予約の作成だけでは送らない
    let account = call(
        &router,
        "POST",
        "/accounts",
        None,
        json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" }),
        StatusCode::CREATED,
    )
    .await;
    let student = account["access_token"].as_str().unwrap();
    let student_id =
        account_id_uuid::parse(account["student"]["student_id"].as_str().unwrap()).unwrap();
    let slot = call(
        &router,
        "POST",
        "/slots",
        Some(&coach),
        json!({ "sport": "tennis", "starts_at": "2026-10-10T10:00:00Z", "ends_at": "2026-10-10T11:00:00Z" }),
        StatusCode::CREATED,
    )
    .await;
    let booking = call(
        &router,
        "POST",
        "/bookings",
        Some(student),
        json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        StatusCode::CREATED,
    )
    .await;
    let booking_id = booking["booking_id"].as_str().unwrap();
    assert!(push.sent().is_empty(), "作成では送らない");

    // 2. 確定で生徒に 1 件届く
    call(
        &router,
        "POST",
        &format!("/bookings/{booking_id}/confirm"),
        Some(student),
        json!({}),
        StatusCode::OK,
    )
    .await;
    let expected = PushNotification::BookingConfirmed {
        student_id,
        coach_id,
        booking_id: booking_id_uuid::parse(booking_id).unwrap(),
    };
    assert_eq!(push.sent(), [expected]);
    assert_eq!(push.sent()[0].recipient(), student_id);

    // 3. キャンセルでは送らない
    call(
        &router,
        "POST",
        &format!("/bookings/{booking_id}/cancel"),
        Some(student),
        json!({}),
        StatusCode::OK,
    )
    .await;
    assert_eq!(push.sent(), [expected], "キャンセルの通知はまだない");
}
