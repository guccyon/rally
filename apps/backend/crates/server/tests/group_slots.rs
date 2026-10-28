//! 変更シナリオ cs-004（グループレッスン）の 1 本目の、予約枠の定員（D40）を、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。定員 3 の枠に 3 人が予約・確定でき、4 人目と同じ生徒の 2 回目は拒まれ、
//! キャンセルで空いた席はまた予約できること。定員を省いた枠は今までのマンツーマンと同じに振る舞うこと。
//! ビデオ通話の暦月の枠（不変条件 1）は今までどおり生徒ごとの予約の確定で 1 回ずつ使う。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test group_slots -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer},
    in_memory::{ManualClock, RecordingAnalyticsSink},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{Role, Sport};
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

/// 生徒の検索に出る枠（テニス）。
async fn search(router: &Router, student: &str) -> Vec<Value> {
    let (status, body) = send(router, "GET", "/slots?sport=tennis", Some(student), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["items"].as_array().unwrap().clone()
}

/// コーチの自分の枠の一覧の 1 件目の枠。
async fn coach_slot(router: &Router, coach: &str) -> Value {
    let (status, body) = send(router, "GET", "/coaches/me/slots", Some(coach), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["slots"][0]["slot"].clone()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn three_students_book_a_slot_of_capacity_three_and_the_fourth_is_rejected(pool: PgPool) {
    let coach = tennis_coach(&pool).await;
    let router = server::router_with_postgres(
        pool,
        ManualClock::new(nov1()),
        RecordingAnalyticsSink::default(),
    );
    let mut students = Vec::new();
    for n in 1..=4 {
        students.push(subscribed_student(&router, n).await);
    }

    let slot = publish(&router, &coach, Some(3)).await;
    assert_eq!(slot["capacity"], 3);
    assert_eq!(slot["remaining_seats"], 3);

    // 3 人までは予約でき、残りの席が減る
    let mut bookings = Vec::new();
    for (i, student) in students[..3].iter().enumerate() {
        let (status, booking) = book(&router, student, &slot).await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        bookings.push(booking);
        let found = search(&router, student).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["remaining_seats"], 3 - (i + 1));
    }
    // 4 人目は満席で拒まれる
    let (status, body) = book(&router, &students[3], &slot).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "slot_already_booked");
    // 同じ生徒の 2 回目も拒まれる
    let (status, body) = book(&router, &students[0], &slot).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "slot_already_booked");

    // 確定した予約が定員に達するまでは公開中で、達したら埋まって検索に出ない
    for (student, booking) in students.iter().zip(&bookings).take(2) {
        confirm(&router, student, booking).await;
    }
    assert_eq!(coach_slot(&router, &coach).await["status"], "open");
    confirm(&router, &students[2], &bookings[2]).await;
    let slot_of_coach = coach_slot(&router, &coach).await;
    assert_eq!(slot_of_coach["status"], "booked");
    assert_eq!(slot_of_coach["remaining_seats"], 0);
    assert!(search(&router, &students[3]).await.is_empty());

    // ビデオ通話の暦月の枠は生徒ごとに 1 回ずつ使う（不変条件 1。数え方は変えない）
    for student in &students[..3] {
        let (status, entitlement) =
            send(&router, "GET", "/entitlements/me", Some(student), None).await;
        assert_eq!(status, StatusCode::OK, "{entitlement}");
        assert_eq!(entitlement["live_lesson_quota"]["used"], 1, "{entitlement}");
    }

    // 1 人がキャンセルすると席が空き、公開中に戻って 4 人目が予約できる
    let path = format!(
        "/bookings/{}/cancel",
        bookings[1]["booking_id"].as_str().unwrap()
    );
    let (status, body) = send(&router, "POST", &path, Some(&students[1]), Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let found = search(&router, &students[3]).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["remaining_seats"], 1);
    let (status, booking) = book(&router, &students[3], &slot).await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn slot_without_a_capacity_is_one_to_one_as_before(pool: PgPool) {
    let coach = tennis_coach(&pool).await;
    let router = server::router_with_postgres(
        pool,
        ManualClock::new(nov1()),
        RecordingAnalyticsSink::default(),
    );
    let (first, second) = (
        subscribed_student(&router, 1).await,
        subscribed_student(&router, 2).await,
    );

    let slot = publish(&router, &coach, None).await;
    assert_eq!(slot["capacity"], 1);
    assert_eq!(slot["remaining_seats"], 1);

    let (status, booking) = book(&router, &first, &slot).await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    // 2 人目は今までどおり 409 slot_already_booked
    let (status, body) = book(&router, &second, &slot).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "slot_already_booked");

    // 確定で埋まり、検索に出なくなる
    confirm(&router, &first, &booking).await;
    let slot_of_coach = coach_slot(&router, &coach).await;
    assert_eq!(slot_of_coach["status"], "booked");
    assert_eq!(slot_of_coach["remaining_seats"], 0);
    assert!(search(&router, &second).await.is_empty());
}
