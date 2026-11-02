//! コーチが自分の予約の一覧を枠の開始の日付で絞る（admin-web の `listMyCoachBookings` の `from`・`to`）ことを、保存先を
//! Postgres にした組み立て（`server::router_with_postgres`）で通す。日付を省けば今までどおり本人の予約をすべて返すこと、
//! 日付を付ければ Postgres の予約を日本時間の日付で絞り、省いた日付を `listBookings` と同じ規則で決めること、期間の違反は
//! 422 になること、別のコーチの予約は出ないことを確かめる。
//! 契約の形・期間の規則の細かい検査は interface の `tests/bookings.rs`、期間に開始する予約の読み取りは
//! infra の `tests/pg_booking_repository.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coach_bookings -- --include-ignored` で走らせる。

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

/// アカウントを登録してトークンを返す。契約にコーチの登録の操作がないので、ユースケースを直に呼ぶ。
async fn register(pool: &PgPool, email: &str, role: Role) -> String {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        SystemClock,
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: email.split('@').next().unwrap().to_string(),
        role,
        sports: vec![Sport::Tennis],
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

/// 一覧の予約の枠の開始時刻
fn starts(body: &Value) -> Vec<String> {
    body["bookings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["slot"]["starts_at"].as_str().unwrap().to_string())
        .collect()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn coach_narrows_own_bookings_by_date_from_postgres(pool: PgPool) {
    let tanaka = register(&pool, "tanaka@example.com", Role::Coach).await;
    let ito = register(&pool, "ito@example.com", Role::Coach).await;
    let taro = register(&pool, "taro@example.com", Role::Student).await;
    let router = server::router_with_postgres(
        pool,
        FixedClock(SystemTime::UNIX_EPOCH + NOW),
        RecordingAnalyticsSink::default(),
    );

    // tanaka の枠: 今日（日本時間の 10 月 26 日 12 時）・11 月 2 日 0 時・11 月 9 日 12 時（日本時間）。ito の枠は 10 月 27 日
    for (coach, starts_at, ends_at) in [
        (&tanaka, "2026-10-26T03:00:00Z", "2026-10-26T04:00:00Z"),
        (&tanaka, "2026-11-01T15:00:00Z", "2026-11-01T16:00:00Z"),
        (&tanaka, "2026-11-09T03:00:00Z", "2026-11-09T04:00:00Z"),
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
        let (status, booking) = send(
            &router,
            post_json(
                "/bookings",
                &taro,
                json!({ "slot_id": slot["slot_id"], "is_trial": false }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
    }

    // 省けば今までどおり本人の予約をすべて返し、期間を載せない
    let (status, body) = send(&router, get("/coaches/me/bookings", &tanaka)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        starts(&body),
        [
            "2026-10-26T03:00:00Z",
            "2026-11-01T15:00:00Z",
            "2026-11-09T03:00:00Z"
        ]
    );
    assert!(
        body.get("from").is_none() && body.get("to").is_none(),
        "{body}"
    );
    assert_eq!(body["bookings"][0]["student_display_name"], "taro");

    for (query, from, to, expected) in [
        // 「来週」の 7 日。11 月 2 日 0 時（日本時間）は入り、9 日は入らない
        (
            "?from=2026-11-02&to=2026-11-08",
            "2026-11-02",
            "2026-11-08",
            vec!["2026-11-01T15:00:00Z"],
        ),
        // to だけなら from は今日。11 月 1 日までには 11 月 2 日 0 時（日本時間）の枠は入らない
        (
            "?to=2026-11-01",
            "2026-10-26",
            "2026-11-01",
            vec!["2026-10-26T03:00:00Z"],
        ),
        // from だけなら to は from から 7 日目
        (
            "?from=2026-11-03",
            "2026-11-03",
            "2026-11-09",
            vec!["2026-11-09T03:00:00Z"],
        ),
    ] {
        let (status, body) = send(
            &router,
            get(&format!("/coaches/me/bookings{query}"), &tanaka),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(body["from"], from, "{query}: {body}");
        assert_eq!(body["to"], to, "{query}: {body}");
        assert_eq!(starts(&body), expected, "{query}: {body}");
    }

    // 別のコーチの予約は期間に入っても出ない
    let (status, body) = send(
        &router,
        get("/coaches/me/bookings?from=2026-10-26&to=2026-11-01", &ito),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(starts(&body), ["2026-10-27T03:00:00Z"]);

    // 期間の違反は listBookings と同じく 422
    for query in ["?to=2026-10-25", "?from=2026-11-01&to=2026-12-02"] {
        let (status, body) = send(
            &router,
            get(&format!("/coaches/me/bookings{query}"), &tanaka),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}: {body}");
    }
}
