//! 変更シナリオ cs-004（グループレッスン）の 2 本目の、グループレッスンの通話と完了（D41）を、保存先を Postgres にした
//! 組み立て（`server::router_with_postgres`）で通す。定員 3 の枠では、コーチが始めた通話セッションに確定した 3 人の予約が
//! 結び付き、ほかの生徒が始めようとすると同じセッションに入る。終了の後、コーチの完了の記録で 3 人分の完了したレッスンが
//! 発行され、ビデオ通話の暦月の枠は生徒ごとに 1 回ずつ使われている。定員 1 の枠は今までのマンツーマンと同じに振る舞う。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test group_live_lessons -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    analytics::AnalyticsEvent,
    identity::{RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer},
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{catalog::Sport, identity::Role};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-11-01T00:00:00Z
fn nov1() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
}

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
    let response = router
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// テニスのコーチを登録してトークンを返す。契約にコーチの登録の操作がないので、ユースケースを直に呼ぶ。
async fn tennis_coach(pool: &PgPool) -> String {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        ManualClock::new(nov1()),
    )
    .execute(RegisterAccountCommand {
        email: "coach@example.com".to_string(),
        display_name: "コーチ".to_string(),
        role: Role::Coach,
        sports: vec![Sport::Tennis],
    })
    .await
    .unwrap();
    MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role: Role::Coach,
        })
        .into_string()
}

/// テニスの生徒を student-app から登録し、月額契約を結んでトークンを返す。
async fn subscribed_student(router: &Router, n: usize) -> String {
    let (status, account) = send(
        router,
        "POST",
        "/accounts",
        None,
        Some(json!({
            "email": format!("student{n}@example.com"),
            "display_name": format!("生徒{n}"),
            "sport": "tennis",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    let student = account["access_token"].as_str().unwrap().to_string();
    let (status, body) = send(
        router,
        "POST",
        "/subscriptions",
        Some(&student),
        Some(json!({ "sport": "tennis", "payment_method_token": "tok_mock" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    student
}

async fn publish(router: &Router, coach: &str, capacity: Option<u8>) -> Value {
    let mut request = json!({
        "sport": "tennis",
        "starts_at": "2026-12-01T01:00:00Z",
        "ends_at": "2026-12-01T02:00:00Z",
    });
    if let Some(capacity) = capacity {
        request["capacity"] = json!(capacity);
    }
    let (status, slot) = send(router, "POST", "/slots", Some(coach), Some(request)).await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    slot
}

async fn book(router: &Router, student: &str, slot: &Value) -> (StatusCode, Value) {
    send(
        router,
        "POST",
        "/bookings",
        Some(student),
        Some(json!({ "slot_id": slot["slot_id"], "is_trial": false })),
    )
    .await
}

async fn confirm(router: &Router, student: &str, booking: &Value) {
    let path = format!(
        "/bookings/{}/confirm",
        booking["booking_id"].as_str().unwrap()
    );
    let (status, body) = send(router, "POST", &path, Some(student), Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

async fn start(router: &Router, token: &str, booking: &Value) -> (StatusCode, Value) {
    send(
        router,
        "POST",
        "/live-lesson-sessions",
        Some(token),
        Some(json!({ "booking_id": booking["booking_id"] })),
    )
    .await
}

async fn end(router: &Router, token: &str, session: &Value) -> (StatusCode, Value) {
    let path = format!(
        "/live-lesson-sessions/{}/end",
        session["session_id"].as_str().unwrap()
    );
    send(router, "POST", &path, Some(token), Some(json!({}))).await
}

async fn complete(router: &Router, token: &str, session: &Value) -> (StatusCode, Value) {
    send(
        router,
        "POST",
        "/lessons/completions",
        Some(token),
        Some(json!({ "lesson_type": "live_lesson", "session_id": session["session_id"] })),
    )
    .await
}

/// `student` のビデオ通話の暦月の枠で使った回数。
async fn quota_used(router: &Router, student: &str) -> Value {
    let (status, entitlement) = send(router, "GET", "/entitlements/me", Some(student), None).await;
    assert_eq!(status, StatusCode::OK, "{entitlement}");
    entitlement["live_lesson_quota"]["used"].clone()
}

/// 発火した `lesson.completed` の生徒（発火した順）。
fn completed_students(analytics: &RecordingAnalyticsSink) -> Vec<String> {
    analytics
        .events()
        .into_iter()
        .filter_map(|event| match event {
            AnalyticsEvent::LessonCompleted { student_id, .. } => {
                Some(application::identity::account_id_uuid::format(student_id))
            }
            _ => None,
        })
        .collect()
}

fn booking_ids(session: &Value) -> Vec<Value> {
    session["bookings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["booking_id"].clone())
        .collect()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_group_lesson_of_three_completes_and_consumes_for_each_student(pool: PgPool) {
    let coach = tennis_coach(&pool).await;
    let analytics = RecordingAnalyticsSink::default();
    let router = server::router_with_postgres(pool, ManualClock::new(nov1()), analytics.clone());
    let mut students = Vec::new();
    for n in 1..=3 {
        students.push(subscribed_student(&router, n).await);
    }
    let slot = publish(&router, &coach, Some(3)).await;
    let mut bookings = Vec::new();
    for student in &students {
        let (status, booking) = book(&router, student, &slot).await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        confirm(&router, student, &booking).await;
        bookings.push(booking);
    }

    // コーチが 1 人目の予約で始めると、確定した 3 人の予約がすべて結び付く（先頭は開始した予約）
    let (status, session) = start(&router, &coach, &bookings[0]).await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    let linked: Vec<Value> = bookings.iter().map(|b| b["booking_id"].clone()).collect();
    assert_eq!(booking_ids(&session), linked);
    assert_eq!(session["booking_id"], bookings[0]["booking_id"]);

    // ほかの予約（とコーチの 2 回目）で始めようとしても、同じセッションに入る
    for (token, booking) in [
        (&students[1], &bookings[1]),
        (&students[2], &bookings[2]),
        (&students[0], &bookings[0]),
        (&coach, &bookings[1]),
    ] {
        let (status, joined) = start(&router, token, booking).await;
        assert_eq!(status, StatusCode::CREATED, "{joined}");
        assert_eq!(joined["session_id"], session["session_id"]);
        assert_eq!(booking_ids(&joined), linked);
        assert!(joined["room_token"].is_string());
    }
    let started = analytics
        .events()
        .iter()
        .filter(|e| e.name() == "call.started")
        .count();
    assert_eq!(started, 1, "call.started は最初の開始の 1 回だけ");

    let (status, ended) = end(&router, &coach, &session).await;
    assert_eq!(status, StatusCode::OK, "{ended}");
    assert_eq!(ended["status"], "ended");
    assert_eq!(booking_ids(&ended), linked);

    // コーチの完了の記録で 3 人分の完了したレッスンが発行される（それぞれの生徒の初回）
    let (status, lesson) = complete(&router, &coach, &session).await;
    assert_eq!(status, StatusCode::CREATED, "{lesson}");
    assert_eq!(lesson["is_first_lesson"], true);
    let student_ids: Vec<String> = session["bookings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["student_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(completed_students(&analytics), student_ids);
    assert_eq!(lesson["student_id"], student_ids[0].as_str());

    // 記録済みの生徒・コーチの 2 回目は 409
    for token in [&students[1], &coach] {
        let (status, body) = complete(&router, token, &session).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["code"], "lesson_already_completed");
    }

    // ビデオ通話の暦月の枠は生徒ごとに 1 回ずつ使っている（不変条件 1）
    for student in &students {
        assert_eq!(quota_used(&router, student).await, 1);
    }
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn students_of_a_group_lesson_each_record_their_own_completion(pool: PgPool) {
    let coach = tennis_coach(&pool).await;
    let analytics = RecordingAnalyticsSink::default();
    let router = server::router_with_postgres(pool, ManualClock::new(nov1()), analytics.clone());
    let (first, second) = (
        subscribed_student(&router, 1).await,
        subscribed_student(&router, 2).await,
    );
    let slot = publish(&router, &coach, Some(3)).await;
    let mut bookings = Vec::new();
    for student in [&first, &second] {
        let (status, booking) = book(&router, student, &slot).await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        confirm(&router, student, &booking).await;
        bookings.push(booking);
    }
    let (status, session) = start(&router, &second, &bookings[1]).await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    let (status, ended) = end(&router, &coach, &session).await;
    assert_eq!(status, StatusCode::OK, "{ended}");

    // 生徒は自分の分だけを記録する
    for (student, booking) in [(&first, &bookings[0]), (&second, &bookings[1])] {
        let (status, lesson) = complete(&router, student, &session).await;
        assert_eq!(status, StatusCode::CREATED, "{lesson}");
        assert_eq!(lesson["student_id"], booking["student_id"]);
        assert_eq!(lesson["session_id"], session["session_id"]);
    }
    assert_eq!(completed_students(&analytics).len(), 2);
    let (status, body) = complete(&router, &coach, &session).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "lesson_already_completed");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_one_to_one_lesson_is_started_and_completed_as_before(pool: PgPool) {
    let coach = tennis_coach(&pool).await;
    let analytics = RecordingAnalyticsSink::default();
    let router = server::router_with_postgres(pool, ManualClock::new(nov1()), analytics.clone());
    let student = subscribed_student(&router, 1).await;
    let slot = publish(&router, &coach, None).await;
    let (status, booking) = book(&router, &student, &slot).await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    confirm(&router, &student, &booking).await;

    let (status, session) = start(&router, &student, &booking).await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    assert_eq!(booking_ids(&session), vec![booking["booking_id"].clone()]);
    // 同じ予約の 2 回目の開始は、コーチからでも今までどおり 409
    let (status, body) = start(&router, &coach, &booking).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "invalid_session_state");

    let (status, ended) = end(&router, &coach, &session).await;
    assert_eq!(status, StatusCode::OK, "{ended}");
    let (status, lesson) = complete(&router, &student, &session).await;
    assert_eq!(status, StatusCode::CREATED, "{lesson}");
    assert_eq!(lesson["student_id"], booking["student_id"]);
    let (status, body) = complete(&router, &coach, &session).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "lesson_already_completed");
    assert_eq!(completed_students(&analytics).len(), 1);
    assert_eq!(quota_used(&router, &student).await, 1);
}
