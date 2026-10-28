//! 変更シナリオ cs-001 で足した水泳（`swimming`）を、保存先を Postgres にした組み立て（`server::router_with_postgres`）で
//! 一周させる。水泳のコーチが枠を公開し、水泳の生徒が月額契約を結んで、その枠を検索・予約・確定できること。
//! 受講権は月額（`subscription`）で、確定でビデオ通話の暦月の枠を 1 回使う（ワンショットにはならない）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test swimming -- --include-ignored` で走らせる。

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

/// 2026-10-05T00:00:00+09:00（2026-10-04T15:00:00Z）
fn oct5_jst() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_126_000)
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

/// 水泳のコーチを登録してトークンを返す。契約にコーチの登録の操作がないので、ユースケースを直に呼ぶ。
async fn swimming_coach(pool: &PgPool) -> String {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        ManualClock::new(oct5_jst()),
    )
    .execute(RegisterAccountCommand {
        email: "coach@example.com".to_string(),
        display_name: "およぐ".to_string(),
        role: Role::Coach,
        sports: vec![Sport::Swimming],
    })
    .await
    .unwrap();
    assert!(account.as_coach().unwrap().offers(Sport::Swimming));
    MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role: Role::Coach,
        })
        .into_string()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn swimming_student_books_a_swimming_coach_under_the_monthly_entitlement(pool: PgPool) {
    let coach = swimming_coach(&pool).await;
    let router = server::router_with_postgres(
        pool,
        ManualClock::new(oct5_jst()),
        RecordingAnalyticsSink::default(),
    );

    // 生徒は student-app から水泳で登録する
    let (status, account) = send(
        &router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "swimming" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    assert_eq!(account["student"]["sport"], "swimming");
    let student = account["access_token"].as_str().unwrap().to_string();

    // 水泳は月額（ゴルフ以外）で契約できる
    let (status, subscription) = send(
        &router,
        "POST",
        "/subscriptions",
        Some(&student),
        Some(json!({ "sport": "swimming", "payment_method_token": "tok_mock" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{subscription}");
    assert_eq!(subscription["subscription"]["sport"], "swimming");
    assert_eq!(subscription["entitlement"]["contract_type"], "subscription");

    // コーチが水泳の枠を公開し、生徒が検索して予約・確定する
    let (status, slot) = send(
        &router,
        "POST",
        "/slots",
        Some(&coach),
        Some(json!({
            "sport": "swimming",
            "starts_at": "2026-12-01T01:00:00Z",
            "ends_at": "2026-12-01T02:00:00Z",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let (status, found) = send(
        &router,
        "GET",
        "/slots?sport=swimming",
        Some(&student),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{found}");
    let found: Vec<&Value> = found["items"].as_array().unwrap().iter().collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["slot_id"], slot["slot_id"]);

    let (status, booking) = send(
        &router,
        "POST",
        "/bookings",
        Some(&student),
        Some(json!({ "slot_id": slot["slot_id"], "is_trial": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    let booking_id = booking["booking_id"].as_str().unwrap();
    let (status, confirmed) = send(
        &router,
        "POST",
        &format!("/bookings/{booking_id}/confirm"),
        Some(&student),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed}");

    // 受講権は月額で、確定でビデオ通話の暦月の枠を 1 回使う（ワンショットではない）
    let (status, entitlement) =
        send(&router, "GET", "/entitlements/me", Some(&student), None).await;
    assert_eq!(status, StatusCode::OK, "{entitlement}");
    assert_eq!(entitlement["contract_type"], "subscription");
    assert_eq!(entitlement["sport"], "swimming");
    assert_eq!(entitlement["unlimited_video_and_chat"], true);
    assert_eq!(entitlement["remaining_video_reviews"], Value::Null);
    assert_eq!(
        entitlement["live_lesson_quota"],
        json!({ "month": "2026-10", "limit": 2, "used": 1, "remaining": 1 })
    );
}
