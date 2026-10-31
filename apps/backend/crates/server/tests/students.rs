//! 本部が見る生徒の一覧（admin-web の `GET /students`）を、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。生徒が student-app の操作で登録し・月額契約を始め・予約したことが、
//! 一覧の契約種別と契約プラン・最後に予約した時刻に出ること、コーチ・生徒は 403 であることを確かめる。契約の形の
//! 細かい検査は interface の `tests/students.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test students -- --include-ignored` で走らせる。

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
use domain::{catalog::Sport, identity::Role};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// 2026-10-05T00:00:00Z から `secs` 秒後（Postgres の時刻はマイクロ秒までなので、秒単位にする）。
fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_791_158_400 + secs)
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

/// 役割 `role` のアカウントをユースケースを直に呼んで登録し（契約にコーチ・本部の登録の操作がない）、トークンを返す。
async fn register(pool: &PgPool, email: &str, role: Role, sports: Vec<Sport>) -> String {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        ManualClock::new(at(0)),
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: "テスト".to_string(),
        role,
        sports,
    })
    .await
    .unwrap();
    MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string()
}

/// student-app の `POST /accounts` で生徒を登録し、生徒の ID とトークンを返す。
async fn register_student(router: &Router, email: &str, sport: &str) -> (String, String) {
    let (status, body) = send(
        router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": email, "display_name": "せいと", "sport": sport })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (
        body["student"]["student_id"].as_str().unwrap().to_string(),
        body["access_token"].as_str().unwrap().to_string(),
    )
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_sees_contract_and_last_booking_of_students(pool: PgPool) {
    let coach = register(&pool, "coach@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let headquarters = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let clock = ManualClock::new(at(60));
    let router =
        server::router_with_postgres(pool, clock.clone(), RecordingAnalyticsSink::default());

    let (subscriber_id, subscriber) = register_student(&router, "taro@example.com", "tennis").await;
    clock.set(at(120));
    let (newcomer_id, _) = register_student(&router, "hanako@example.com", "swimming").await;

    // 先に登録した生徒は月額（plus）を始め、予約する
    clock.set(at(180));
    let (status, body) = send(
        &router,
        "POST",
        "/subscriptions",
        Some(&subscriber),
        Some(json!({ "sport": "tennis", "plan": "plus", "payment_method_token": "tok_mock" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, slot) = send(
        &router,
        "POST",
        "/slots",
        Some(&coach),
        Some(json!({
            "sport": "tennis",
            "starts_at": "2099-01-01T10:00:00Z",
            "ends_at": "2099-01-01T11:00:00Z",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    clock.set(at(240));
    let (status, body) = send(
        &router,
        "POST",
        "/bookings",
        Some(&subscriber),
        Some(json!({ "slot_id": slot["slot_id"], "is_trial": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = send(&router, "GET", "/students", Some(&headquarters), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "students": [
                {
                    "student_id": newcomer_id,
                    "display_name": "せいと",
                    "sport": "swimming",
                    "contract_type": null,
                    "plan": null,
                    "last_booked_at": null,
                    "registered_at": "2026-10-05T00:02:00Z",
                },
                {
                    "student_id": subscriber_id,
                    "display_name": "せいと",
                    "sport": "tennis",
                    "contract_type": "subscription",
                    "plan": "plus",
                    "last_booked_at": "2026-10-05T00:04:00Z",
                    "registered_at": "2026-10-05T00:01:00Z",
                },
            ],
        })
    );

    for token in [&coach, &subscriber] {
        let (status, body) = send(&router, "GET", "/students", Some(token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
}
