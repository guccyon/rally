//! 生徒と担当コーチのチャットを、組み立て済みのルーターで一周させる（API の E2E）。スレッドは予約の確定で
//! messaging の購読（`MessageThreadSubscriber`）が開き、生徒が送ってよいかは entitlement（`EntitlementChatGate`）に
//! 問い合わせる。
//!
//! 1. 生徒の登録 → コーチが枠を公開 → 生徒が体験予約して確定 → 生徒とコーチのスレッドが 1 つ開く
//! 2. 月額契約の前の生徒は送れない（403 `chat_not_entitled`）が、コーチは送れる
//! 3. 生徒が月額契約 → 送り放題で送れる（受講権の通話の枠は変わらない）→ コーチが既読にして返信 → 生徒が読む
//! 4. 同じコーチとの 2 件目の予約を確定してもスレッドは 1 つのまま
//! 5. 生徒が動画を送り（添削依頼）、コーチが添削を返すと、同じスレッドに依頼と返却のメッセージが並ぶ
//!    （async-lesson の `VideoReviewRequested` / `VideoReviewReturned` を messaging が購読する）。依頼は種類
//!    `video_review_request` と動画の ID、返却は `video_review_returned` と動画の ID・添削の ID で返る
//! 6. 生徒とコーチがスタンプを送る（`stamp_id`）。種類 `stamp` とスタンプの ID、古い版のクライアント向けの本文の説明
//!    （「スタンプ: ありがとう」）で返り、保存先から読み直しても同じ。知らない ID は 422
//!
//! 保存先がインメモリの組み立て（`server::router_at`）と、Postgres の組み立て（`server::router_with_postgres`。
//! `#[ignore]`、`DATABASE_URL` を設定して `--include-ignored` で走らせる）の両方で同じ一周を通す。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role, Sport};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-05T00:00:00Z（シナリオの始まり）
fn oct5() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_158_400)
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn call(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
    expected: StatusCode,
) -> Value {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let (status, body) = send(router, request).await;
    assert_eq!(status, expected, "{method} {path}: {body}");
    body
}

fn coach_token(coach_id: AccountId) -> String {
    MockTokenService
        .issue(TokenClaims {
            account_id: coach_id,
            role: Role::Coach,
        })
        .into_string()
}

/// コーチのトークン `coach` で枠を公開し、生徒 `student` が予約して確定する。予約の ID を返す。
async fn book_and_confirm(
    router: &Router,
    coach: &str,
    student: &str,
    starts_at: &str,
    ends_at: &str,
    is_trial: bool,
) -> String {
    let slot = call(
        router,
        "POST",
        "/slots",
        Some(coach),
        Some(json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at })),
        StatusCode::CREATED,
    )
    .await;
    let booking = call(
        router,
        "POST",
        "/bookings",
        Some(student),
        Some(json!({ "slot_id": slot["slot_id"], "is_trial": is_trial })),
        StatusCode::CREATED,
    )
    .await;
    let booking_id = booking["booking_id"].as_str().unwrap().to_string();
    call(
        router,
        "POST",
        &format!("/bookings/{booking_id}/confirm"),
        Some(student),
        Some(json!({})),
        StatusCode::OK,
    )
    .await;
    booking_id
}

/// 上の 1〜4 の一周。`coach_id` は登録済み（インメモリでは任意）のコーチ。
async fn chat_round_trip(
    router: Router,
    clock: ManualClock,
    analytics: RecordingAnalyticsSink,
    coach_id: AccountId,
) {
    let coach = coach_token(coach_id);
    let account = call(
        &router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })),
        StatusCode::CREATED,
    )
    .await;
    let student = account["access_token"].as_str().unwrap().to_string();
    let student_id = account["student"]["student_id"]
        .as_str()
        .unwrap()
        .to_string();

    // 1. 確定前はスレッドがなく、体験予約の確定でスレッドが開く
    let threads = call(
        &router,
        "GET",
        "/message-threads",
        Some(&student),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(threads["threads"], json!([]));
    book_and_confirm(
        &router,
        &coach,
        &student,
        "2026-10-10T10:00:00Z",
        "2026-10-10T11:00:00Z",
        true,
    )
    .await;
    let threads = call(
        &router,
        "GET",
        "/message-threads",
        Some(&student),
        None,
        StatusCode::OK,
    )
    .await;
    let threads = threads["threads"].as_array().unwrap().clone();
    assert_eq!(threads.len(), 1, "{threads:?}");
    assert_eq!(threads[0]["student_id"], student_id);
    assert_eq!(threads[0]["coach_id"], account_id_uuid::format(coach_id));
    assert_eq!(threads[0]["opened_at"], "2026-10-05T00:00:00Z");
    let thread_id = threads[0]["thread_id"].as_str().unwrap().to_string();
    let messages_path = format!("/message-threads/{thread_id}/messages");
    let read_path = format!("/message-threads/{thread_id}/read");

    // 2. 月額契約の前の生徒は送れないが、コーチは送れる
    clock.set(oct5() + Duration::from_secs(60));
    let denied = call(
        &router,
        "POST",
        &messages_path,
        Some(&student),
        Some(json!({ "body": "体験の前に質問です" })),
        StatusCode::FORBIDDEN,
    )
    .await;
    assert_eq!(denied["code"], "chat_not_entitled");
    let welcome = call(
        &router,
        "POST",
        &messages_path,
        Some(&coach),
        Some(json!({ "body": "体験のご予約ありがとうございます" })),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(welcome["sender_role"], "coach");

    // 3. 月額契約の後は送り放題（通話の枠は変わらない）
    call(
        &router,
        "POST",
        "/subscriptions",
        Some(&student),
        Some(json!({ "sport": "tennis", "payment_method_token": "tok_mock" })),
        StatusCode::CREATED,
    )
    .await;
    let quota_before = call(
        &router,
        "GET",
        "/entitlements/me",
        Some(&student),
        None,
        StatusCode::OK,
    )
    .await;
    clock.set(oct5() + Duration::from_secs(120));
    for text in [
        "よろしくお願いします",
        "フォアハンドが安定しません",
        "動画も送ります",
    ] {
        call(
            &router,
            "POST",
            &messages_path,
            Some(&student),
            Some(json!({ "body": text })),
            StatusCode::CREATED,
        )
        .await;
    }
    let quota_after = call(
        &router,
        "GET",
        "/entitlements/me",
        Some(&student),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(quota_after, quota_before, "送信で受講権は変わらない");

    let coach_threads = call(
        &router,
        "GET",
        "/message-threads",
        Some(&coach),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(coach_threads["threads"][0]["unread_count"], 3);
    clock.set(oct5() + Duration::from_secs(600));
    let read = call(
        &router,
        "POST",
        &read_path,
        Some(&coach),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(read["unread_count"], 0);
    call(
        &router,
        "POST",
        &messages_path,
        Some(&coach),
        Some(json!({ "body": "ラケットを早めに引きましょう" })),
        StatusCode::CREATED,
    )
    .await;

    let list = call(
        &router,
        "GET",
        &messages_path,
        Some(&student),
        None,
        StatusCode::OK,
    )
    .await;
    let messages = list["messages"].as_array().unwrap();
    assert_eq!(
        messages
            .iter()
            .map(|m| (
                m["sender_role"].as_str().unwrap(),
                m["body"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        [
            ("coach", "体験のご予約ありがとうございます"),
            ("student", "よろしくお願いします"),
            ("student", "フォアハンドが安定しません"),
            ("student", "動画も送ります"),
            ("coach", "ラケットを早めに引きましょう"),
        ]
    );
    for m in &messages[1..4] {
        assert_eq!(m["read_at"], "2026-10-05T00:10:00Z", "{m}");
    }
    let read = call(
        &router,
        "POST",
        &read_path,
        Some(&student),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(read["unread_count"], 0);

    // 4. 同じコーチとの 2 件目の予約を確定してもスレッドは 1 つ
    book_and_confirm(
        &router,
        &coach,
        &student,
        "2026-10-12T10:00:00Z",
        "2026-10-12T11:00:00Z",
        false,
    )
    .await;
    let threads = call(
        &router,
        "GET",
        "/message-threads",
        Some(&student),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(threads["threads"].as_array().unwrap().len(), 1);
    assert_eq!(threads["threads"][0]["thread_id"], thread_id);

    let sent: Vec<AnalyticsEvent> = analytics
        .events()
        .into_iter()
        .filter(|e| e.name() == "message.sent")
        .collect();
    assert_eq!(sent.len(), 5, "送れた 5 通だけ発火する");

    // 5. 添削依頼と添削の返却が同じスレッドに並ぶ（message.sent は発火しない）
    clock.set(oct5() + Duration::from_secs(3600));
    let video = call(
        &router,
        "POST",
        "/videos",
        Some(&student),
        Some(json!({
            "coach_id": account_id_uuid::format(coach_id),
            "sport": "tennis",
            "content_type": "video/mp4"
        })),
        StatusCode::CREATED,
    )
    .await;
    let video_id = video["video_id"].as_str().unwrap();
    let coach_threads = call(
        &router,
        "GET",
        "/message-threads",
        Some(&coach),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(coach_threads["threads"].as_array().unwrap().len(), 1);
    assert_eq!(coach_threads["threads"][0]["unread_count"], 1, "届いた依頼");
    assert_eq!(
        coach_threads["threads"][0]["last_message"]["kind"],
        "video_review_request"
    );
    assert_eq!(
        coach_threads["threads"][0]["last_message"]["video_id"],
        video_id
    );
    clock.set(oct5() + Duration::from_secs(7200));
    let review = call(
        &router,
        "POST",
        &format!("/videos/{video_id}/review"),
        Some(&coach),
        Some(json!({ "comment": "フォロースルーを大きく" })),
        StatusCode::OK,
    )
    .await;
    let list = call(
        &router,
        "GET",
        &messages_path,
        Some(&student),
        None,
        StatusCode::OK,
    )
    .await;
    let messages = list["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 7);
    assert!(
        messages[..5].iter().all(|m| m["kind"] == "text"),
        "当事者が送ったものは本文"
    );
    let request = &messages[5];
    assert_eq!(request["sender_role"], "student");
    assert_eq!(request["sender_id"], student_id);
    assert_eq!(request["kind"], "video_review_request");
    assert_eq!(request["video_id"], video_id);
    assert_eq!(request.get("review_id"), None);
    assert_eq!(request["body"], "動画の添削を依頼しました");
    assert_eq!(request["sent_at"], "2026-10-05T01:00:00Z");
    let returned = &messages[6];
    assert_eq!(returned["sender_role"], "coach");
    assert_eq!(returned["sender_id"], account_id_uuid::format(coach_id));
    assert_eq!(returned["kind"], "video_review_returned");
    assert_eq!(returned["video_id"], video_id);
    assert_eq!(returned["review_id"], review["review_id"]);
    assert_eq!(returned["body"], "動画の添削を返しました");
    assert_eq!(returned["sent_at"], "2026-10-05T02:00:00Z");
    assert_eq!(returned["read_at"], Value::Null);
    let sent = analytics
        .events()
        .into_iter()
        .filter(|e| e.name() == "message.sent")
        .count();
    assert_eq!(sent, 5, "添削依頼・返却は当事者の送信ではない");

    // 6. スタンプ
    clock.set(oct5() + Duration::from_secs(7260));
    for (token, stamp_id) in [(&student, "thanks"), (&coach, "nice_shot")] {
        let stamp = call(
            &router,
            "POST",
            &messages_path,
            Some(token),
            Some(json!({ "stamp_id": stamp_id })),
            StatusCode::CREATED,
        )
        .await;
        assert_eq!(stamp["kind"], "stamp");
        assert_eq!(stamp["stamp_id"], stamp_id);
    }
    call(
        &router,
        "POST",
        &messages_path,
        Some(&student),
        Some(json!({ "stamp_id": "heart" })),
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;
    let list = call(
        &router,
        "GET",
        &messages_path,
        Some(&coach),
        None,
        StatusCode::OK,
    )
    .await;
    let messages = list["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 9);
    let thanks = &messages[7];
    assert_eq!(thanks["sender_role"], "student");
    assert_eq!(thanks["sender_id"], student_id);
    assert_eq!(thanks["kind"], "stamp");
    assert_eq!(thanks["stamp_id"], "thanks");
    assert_eq!(thanks["body"], "スタンプ: ありがとう");
    assert_eq!(thanks["sent_at"], "2026-10-05T02:01:00Z");
    assert_eq!(thanks.get("video_id"), None);
    let nice_shot = &messages[8];
    assert_eq!(nice_shot["sender_role"], "coach");
    assert_eq!(nice_shot["stamp_id"], "nice_shot");
    assert_eq!(nice_shot["body"], "スタンプ: ナイスショット");
    let sent = analytics
        .events()
        .into_iter()
        .filter(|e| e.name() == "message.sent")
        .count();
    assert_eq!(sent, 7, "スタンプは当事者の送信");
}

#[tokio::test]
async fn chat_round_trip_in_memory() {
    let clock = ManualClock::new(oct5());
    let analytics = RecordingAnalyticsSink::default();
    let router = server::router_with_analytics(clock.clone(), analytics.clone());
    chat_round_trip(router, clock, analytics, AccountId::from_u128(42)).await;
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn chat_round_trip_with_postgres(pool: PgPool) {
    let clock = ManualClock::new(oct5());
    let analytics = RecordingAnalyticsSink::default();
    // 契約にコーチの登録の操作がないので、同じ DB にユースケースで登録する
    let coach = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        analytics.clone(),
        clock.clone(),
    )
    .execute(RegisterAccountCommand {
        email: "coach.hanako@example.com".to_string(),
        display_name: "はなこコーチ".to_string(),
        role: Role::Coach,
        sports: vec![Sport::Tennis],
    })
    .await
    .unwrap();
    let router = server::router_with_postgres(pool, clock.clone(), analytics.clone());
    chat_round_trip(router, clock, analytics, coach.id()).await;
}
