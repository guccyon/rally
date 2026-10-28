//! 予約の確定でのプッシュ通知を、組み立て済みのルーターで確かめる（API の E2E）。通知は notification の購読
//! （`PushNotificationSubscriber`）が境界間イベント `BookingConfirmed` で送り、scheduling は notification を直接呼ばない。
//!
//! 1. 生徒の登録 → コーチが枠を公開 → 生徒が予約（作成だけでは送らない）
//! 2. 予約の確定 → 生徒に `BookingConfirmed`、コーチに `BookingReceived` の通知が 1 件ずつ届く（送り先はモック
//!    `MockPushSender`）
//! 3. 予約のキャンセルでは送らない（キャンセルの通知はまだない）
//!
//! 添削の返却（async-lesson の `VideoReviewReturned`）では動画を送った生徒に、メッセージの送信（messaging の
//! `MessageSent`）では受け手（送り手でない側の当事者）に届く。
//!
//! 受け取る人が通知の受け取り設定（`PUT /notification-settings/me`）でその種類をオフにしていれば送らない。受け取り設定が
//! Postgres に保存されることは `router_with_postgres` の組み立てで確かめる（`#[ignore]`。`DATABASE_URL` が要る）。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::{video_id_uuid, video_review_id_uuid},
    identity::{TokenClaims, TokenIssuer, account_id_uuid},
    in_memory::{ManualClock, RecordingAnalyticsSink},
    messaging::{message_id_uuid, message_thread_id_uuid},
    scheduling::booking_id_uuid,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    identity::{AccountId, Role},
    messaging::MessageSender,
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
async fn confirming_a_booking_pushes_a_notification_to_the_student_and_the_coach() {
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

    // 2. 確定で生徒とコーチに 1 件ずつ届く
    call(
        &router,
        "POST",
        &format!("/bookings/{booking_id}/confirm"),
        Some(student),
        json!({}),
        StatusCode::OK,
    )
    .await;
    let booking_id = booking_id_uuid::parse(booking_id).unwrap();
    let expected = [
        PushNotification::BookingConfirmed {
            student_id,
            coach_id,
            booking_id,
        },
        PushNotification::BookingReceived {
            student_id,
            coach_id,
            booking_id,
        },
    ];
    assert_eq!(push.sent(), expected);
    assert_eq!(
        push.sent()
            .iter()
            .map(PushNotification::recipient)
            .collect::<Vec<_>>(),
        [student_id, coach_id]
    );
    let booking_id = booking_id_uuid::format(booking_id);

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
    assert_eq!(push.sent(), expected, "キャンセルの通知はまだない");
}

/// 通知の送り先をモックにした組み立てと、コーチ 42 のトークン・登録した生徒（トークンとアカウント）。
async fn setup() -> (Router, MockPushSender, AccountId, String, AccountId, String) {
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
    let account = call(
        &router,
        "POST",
        "/accounts",
        None,
        json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" }),
        StatusCode::CREATED,
    )
    .await;
    let student = account["access_token"].as_str().unwrap().to_string();
    let student_id =
        account_id_uuid::parse(account["student"]["student_id"].as_str().unwrap()).unwrap();
    (router, push, coach_id, coach, student_id, student)
}

#[tokio::test]
async fn returning_a_video_review_pushes_a_notification_to_the_student() {
    let (router, push, coach_id, coach, student_id, student) = setup().await;

    // 動画の送付では送らない
    let video = call(
        &router,
        "POST",
        "/videos",
        Some(&student),
        json!({
            "coach_id": account_id_uuid::format(coach_id),
            "sport": "tennis",
            "content_type": "video/quicktime"
        }),
        StatusCode::CREATED,
    )
    .await;
    let video_id = video["video_id"].as_str().unwrap();
    assert!(push.sent().is_empty(), "送付では送らない");

    // コーチの返却で生徒に 1 件届く
    let review = call(
        &router,
        "POST",
        &format!("/videos/{video_id}/review"),
        Some(&coach),
        json!({ "comment": "フォロースルーを大きく" }),
        StatusCode::OK,
    )
    .await;
    let expected = PushNotification::VideoReviewReturned {
        student_id,
        coach_id,
        video_id: video_id_uuid::parse(video_id).unwrap(),
        review_id: video_review_id_uuid::parse(review["review_id"].as_str().unwrap()).unwrap(),
    };
    assert_eq!(push.sent(), [expected]);
    assert_eq!(push.sent()[0].recipient(), student_id);
}

#[tokio::test]
async fn a_sent_message_pushes_a_notification_to_the_other_party() {
    let (router, push, coach_id, coach, student_id, student) = setup().await;

    // 体験予約の確定でスレッドが開く（確定の通知が生徒とコーチに 1 件ずつ）
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
        Some(&student),
        json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        StatusCode::CREATED,
    )
    .await;
    call(
        &router,
        "POST",
        &format!(
            "/bookings/{}/confirm",
            booking["booking_id"].as_str().unwrap()
        ),
        Some(&student),
        json!({}),
        StatusCode::OK,
    )
    .await;
    let threads = call(
        &router,
        "GET",
        "/message-threads",
        Some(&student),
        Value::Null,
        StatusCode::OK,
    )
    .await;
    let thread_id = threads["threads"][0]["thread_id"].as_str().unwrap();
    let messages_path = format!("/message-threads/{thread_id}/messages");
    assert_eq!(push.sent().len(), 2, "確定の通知だけ");

    let received = |message: &Value, sender| PushNotification::MessageReceived {
        student_id,
        coach_id,
        thread_id: message_thread_id_uuid::parse(thread_id).unwrap(),
        message_id: message_id_uuid::parse(message["message_id"].as_str().unwrap()).unwrap(),
        sender,
    };

    // コーチが送ると生徒に届く
    let welcome = call(
        &router,
        "POST",
        &messages_path,
        Some(&coach),
        json!({ "body": "体験のご予約ありがとうございます" }),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(push.sent()[2..], [received(&welcome, MessageSender::Coach)]);
    assert_eq!(push.sent()[2].recipient(), student_id);

    // 送れなかった送信（月額契約の前の生徒）では送らない
    call(
        &router,
        "POST",
        &messages_path,
        Some(&student),
        json!({ "body": "質問です" }),
        StatusCode::FORBIDDEN,
    )
    .await;
    assert_eq!(push.sent().len(), 3);

    // 月額契約の後、生徒が送るとコーチに届く
    call(
        &router,
        "POST",
        "/subscriptions",
        Some(&student),
        json!({ "sport": "tennis", "payment_method_token": "tok_mock" }),
        StatusCode::CREATED,
    )
    .await;
    let question = call(
        &router,
        "POST",
        &messages_path,
        Some(&student),
        json!({ "body": "フォアハンドが安定しません" }),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(
        push.sent()[2..],
        [
            received(&welcome, MessageSender::Coach),
            received(&question, MessageSender::Student),
        ]
    );
    assert_eq!(push.sent()[3].recipient(), coach_id);
}

#[tokio::test]
async fn a_kind_the_student_turned_off_is_not_pushed_to_the_student() {
    let (router, push, coach_id, coach, student_id, student) = setup().await;

    // 生徒が予約の確定の通知をオフにする
    let settings = call(
        &router,
        "PUT",
        "/notification-settings/me",
        Some(&student),
        json!({ "booking_confirmed": false }),
        StatusCode::OK,
    )
    .await;
    assert_eq!(settings["booking_confirmed"], false);

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
        Some(&student),
        json!({ "slot_id": slot["slot_id"], "is_trial": true }),
        StatusCode::CREATED,
    )
    .await;
    let booking_id = booking["booking_id"].as_str().unwrap();
    call(
        &router,
        "POST",
        &format!("/bookings/{booking_id}/confirm"),
        Some(&student),
        json!({}),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        push.sent(),
        [PushNotification::BookingReceived {
            student_id,
            coach_id,
            booking_id: booking_id_uuid::parse(booking_id).unwrap(),
        }],
        "生徒には送らず、コーチ（受け取り設定はオンのまま）には送る"
    );
}

/// 保存先を Postgres にした組み立てで、受け取り設定の変更が保存されて次の照会に出ることを確かめる。
///
/// Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
/// `cargo test -p server --test notifications -- --include-ignored` で走らせる。
#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn notification_settings_are_saved_in_postgres(pool: sqlx::PgPool) {
    let router = server::router_with_postgres(
        pool,
        ManualClock::new(oct5()),
        RecordingAnalyticsSink::default(),
    );
    let account = call(
        &router,
        "POST",
        "/accounts",
        None,
        json!({ "email": "hanako@example.com", "display_name": "はなこ", "sport": "soccer" }),
        StatusCode::CREATED,
    )
    .await;
    let student = account["access_token"].as_str().unwrap();
    let all_on = json!({
        "student_id": account["student"]["student_id"],
        "booking_confirmed": true,
        "video_review_returned": true,
        "message_received": true,
    });
    assert_eq!(
        call(
            &router,
            "GET",
            "/notification-settings/me",
            Some(student),
            Value::Null,
            StatusCode::OK
        )
        .await,
        all_on,
        "変えていなければすべてオン"
    );

    let mut expected = all_on;
    expected["video_review_returned"] = json!(false);
    assert_eq!(
        call(
            &router,
            "PUT",
            "/notification-settings/me",
            Some(student),
            json!({ "video_review_returned": false }),
            StatusCode::OK,
        )
        .await,
        expected
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/notification-settings/me",
            Some(student),
            Value::Null,
            StatusCode::OK
        )
        .await,
        expected
    );
}
