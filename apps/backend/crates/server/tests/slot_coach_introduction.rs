//! スロットの検索（student-app の `GET /slots`）に添えるコーチの紹介（`coach_display_name`・`coach_bio`）を、保存先を
//! Postgres にした組み立て（`server::router_with_postgres`）で通す。コーチが `PUT /me/profile` で書いた自己紹介文が
//! 生徒の検索に出ること、自己紹介文を書いていないコーチの枠には `coach_bio` が載らないことを確かめる。
//! 契約の形の細かい検査は interface の `tests/slots.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test slot_coach_introduction -- --include-ignored` で走らせる。

use application::{
    clock::SystemClock,
    identity::{RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer},
    in_memory::RecordingAnalyticsSink,
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

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json");
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

/// アカウントを登録してトークンを返す。契約にコーチの登録の操作がないので、ユースケースを直に呼ぶ。
async fn register(
    pool: &PgPool,
    email: &str,
    display_name: &str,
    role: Role,
    sport: Sport,
) -> String {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        SystemClock,
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: display_name.to_string(),
        role,
        sports: vec![sport],
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

/// コーチ `token` のテニスの枠（2099-01-01 の `hour` 時から 1 時間）を公開し、slot_id を返す。
async fn publish(router: &Router, token: &str, hour: u32) -> String {
    let (status, body) = send(
        router,
        "POST",
        "/slots",
        token,
        Some(json!({
            "sport": "tennis",
            "starts_at": format!("2099-01-01T{hour:02}:00:00Z"),
            "ends_at": format!("2099-01-01T{:02}:00:00Z", hour + 1),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["slot_id"].as_str().unwrap().to_string()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn search_shows_the_coach_display_name_and_bio_from_postgres(pool: PgPool) {
    let with_bio = register(
        &pool,
        "tanaka@example.com",
        "たなか",
        Role::Coach,
        Sport::Tennis,
    )
    .await;
    let without_bio = register(
        &pool,
        "ito@example.com",
        "いとう",
        Role::Coach,
        Sport::Tennis,
    )
    .await;
    let student = register(
        &pool,
        "taro@example.com",
        "たろう",
        Role::Student,
        Sport::Tennis,
    )
    .await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());

    let (status, body) = send(
        &router,
        "PUT",
        "/me/profile",
        &with_bio,
        Some(json!({ "bio": "テニス歴 20 年。初心者歓迎" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let first = publish(&router, &with_bio, 10).await;
    let second = publish(&router, &without_bio, 11).await;

    let (status, body) = send(&router, "GET", "/slots?sport=tennis", &student, None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{body}");
    assert_eq!(items[0]["slot_id"], first);
    assert_eq!(items[0]["coach_display_name"], "たなか");
    assert_eq!(items[0]["coach_bio"], "テニス歴 20 年。初心者歓迎");
    assert_eq!(items[1]["slot_id"], second);
    assert_eq!(items[1]["coach_display_name"], "いとう");
    assert!(
        items[1].get("coach_bio").is_none(),
        "自己紹介文がないコーチは項目なし: {}",
        items[1]
    );

    // 自己紹介文を消すと、その枠からも項目がなくなる
    let (status, body) = send(
        &router,
        "PUT",
        "/me/profile",
        &with_bio,
        Some(json!({ "bio": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, body) = send(&router, "GET", "/slots?sport=tennis", &student, None).await;
    assert!(body["items"][0].get("coach_bio").is_none(), "{body}");
    assert_eq!(body["items"][0]["coach_display_name"], "たなか");
}
