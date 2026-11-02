//! コーチが自分の枠の一覧を枠の開始の日付で絞る（admin-web の `listMyCoachSlots` の `from`・`to`）ことを、保存先を
//! Postgres にした組み立て（`server::router_with_postgres`）で通す。日付を省けば今までどおりすべての枠を返すこと、
//! Postgres の枠を日本時間の日付で絞って開始の古い順に読むこと、省いた側の決め方と期間の違反の 422 が `listBookings` と
//! 同じことを確かめる。
//! 契約の形・期間の規則の細かい検査は interface の `tests/bookings.rs`、コーチの枠の読み取りは infra の
//! `tests/pg_slot_repository.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coach_slots -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    clock::SystemClock,
    identity::{RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer},
    in_memory::{FixedClock, RecordingAnalyticsSink},
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

/// 2026-10-26T00:00:00Z（日本時間の 10 月 26 日 9 時）
const NOW: Duration = Duration::from_secs(1_792_972_800);

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn get(path: &str, token: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn post_json(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// コーチを登録してトークンを返す。契約にコーチの登録の操作がないので、ユースケースを直に呼ぶ。
async fn register_coach(pool: &PgPool, email: &str) -> String {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        SystemClock,
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: email.split('@').next().unwrap().to_string(),
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

fn starts(body: &Value) -> Vec<&str> {
    body["slots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["slot"]["starts_at"].as_str().unwrap())
        .collect()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn coach_narrows_own_slots_by_date_from_postgres(pool: PgPool) {
    let tanaka = register_coach(&pool, "tanaka@example.com").await;
    let ito = register_coach(&pool, "ito@example.com").await;
    let router = server::router_with_postgres(
        pool,
        FixedClock(SystemTime::UNIX_EPOCH + NOW),
        RecordingAnalyticsSink::default(),
    );

    // 日本時間で 10 月 26 日 12 時・11 月 1 日 23 時・11 月 2 日 0 時・11 月 9 日 10 時に始まる枠と、別のコーチの 10 月 27 日の枠
    for (coach, starts_at, ends_at) in [
        (&tanaka, "2026-11-01T15:00:00Z", "2026-11-01T16:00:00Z"),
        (&tanaka, "2026-10-26T03:00:00Z", "2026-10-26T04:00:00Z"),
        (&tanaka, "2026-11-01T14:00:00Z", "2026-11-01T15:00:00Z"),
        (&tanaka, "2026-11-09T01:00:00Z", "2026-11-09T02:00:00Z"),
        (&ito, "2026-10-27T03:00:00Z", "2026-10-27T04:00:00Z"),
    ] {
        let (status, slot) = send(
            &router,
            post_json(
                "/slots",
                coach,
                json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
    }

    for (query, expected) in [
        // 省けば今までどおりすべて（期間で絞らない）
        (
            "",
            vec![
                "2026-10-26T03:00:00Z",
                "2026-11-01T14:00:00Z",
                "2026-11-01T15:00:00Z",
                "2026-11-09T01:00:00Z",
            ],
        ),
        // 11 月 2 日 0 時（日本時間）の枠は 11 月 1 日に入らない
        (
            "?from=2026-11-01&to=2026-11-01",
            vec!["2026-11-01T14:00:00Z"],
        ),
        // from を省けば今日（10 月 26 日）から
        (
            "?to=2026-11-01",
            vec!["2026-10-26T03:00:00Z", "2026-11-01T14:00:00Z"],
        ),
        // to を省けば from から 7 日目（11 月 8 日）まで
        ("?from=2026-11-02", vec!["2026-11-01T15:00:00Z"]),
        ("?from=2026-11-20&to=2026-11-30", vec![]),
    ] {
        let (status, body) =
            send(&router, get(&format!("/coaches/me/slots{query}"), &tanaka)).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(starts(&body), expected, "{query}");
    }

    // 期間の違反は listBookings と同じく 422
    for query in [
        "?from=2026-11-02&to=2026-11-01",
        "?to=2026-10-25",
        "?from=2026-11-01&to=2026-12-02",
        "?from=2026/11/01",
    ] {
        let (status, body) =
            send(&router, get(&format!("/coaches/me/slots{query}"), &tanaka)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
    }
}
