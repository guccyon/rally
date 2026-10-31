//! 予約枠にコーチのレッスンメニューを結びつける（D61）を、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。コーチが admin-web の操作で作ったメニューに枠を結びつけて公開し、
//! 生徒の検索（`searchSlots`）とコーチの自分の枠の一覧（`listMyCoachSlots`）にメニューの ID・名前・時間が載ること、
//! 枠に合わないメニューは 422 で保存しないこと、予約の作成は今までどおりであることを確かめる。契約の形の細かい検査は
//! interface の `tests/slots.rs` と `tests/bookings.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test slot_lesson_menus -- --include-ignored` で走らせる。

use std::time::{Duration, SystemTime};

use application::{
    clock::SystemClock,
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

/// 2026-10-30T00:00:00Z から `secs` 秒後（Postgres の時刻はマイクロ秒までなので、読み戻しと比べられる秒単位にする）。
fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_318_400 + secs)
}

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

/// 役割 `role` のアカウントを登録して、アカウント ID とトークンを返す。契約にコーチ・本部の登録の操作がないので、
/// ユースケースを直に呼ぶ。
async fn register(pool: &PgPool, email: &str, role: Role, sports: Vec<Sport>) -> (String, String) {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        SystemClock,
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: "テスト".to_string(),
        role,
        sports,
    })
    .await
    .unwrap();
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string();
    (
        application::identity::account_id_uuid::format(account.id()),
        token,
    )
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn slot_bound_to_a_lesson_menu_shows_the_menu_to_students_and_the_coach(pool: PgPool) {
    let (_, coach) = register(&pool, "coach@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let (_, student) = register(
        &pool,
        "taro@example.com",
        Role::Student,
        vec![Sport::Tennis],
    )
    .await;
    let router = server::router_with_postgres(
        pool,
        ManualClock::new(at(0)),
        RecordingAnalyticsSink::default(),
    );

    let (status, menu) = send(
        &router,
        "POST",
        "/coaches/me/lesson-menus",
        &coach,
        Some(json!({
            "sport": "tennis",
            "lesson_type": "live_lesson",
            "name": "フォア改善",
            "duration_minutes": 45,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{menu}");
    let publish = |starts_at: &str, ends_at: &str, lesson_menu_id: Option<&Value>| {
        let mut body = json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at });
        if let Some(id) = lesson_menu_id {
            body["lesson_menu_id"] = id.clone();
        }
        send(&router, "POST", "/slots", &coach, Some(body))
    };

    // 枠の長さ（60 分）がメニューの時間（45 分）と違えば 422 で、保存しない
    let (status, body) = publish(
        "2026-11-01T10:00:00Z",
        "2026-11-01T11:00:00Z",
        Some(&menu["lesson_menu_id"]),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    let (status, bound) = publish(
        "2026-11-01T10:00:00Z",
        "2026-11-01T10:45:00Z",
        Some(&menu["lesson_menu_id"]),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bound}");
    let (status, plain) = publish("2026-11-01T12:00:00Z", "2026-11-01T13:00:00Z", None).await;
    assert_eq!(status, StatusCode::CREATED, "{plain}");

    let summary = json!({
        "lesson_menu_id": menu["lesson_menu_id"],
        "name": "フォア改善",
        "duration_minutes": 45,
    });
    let (status, page) = send(&router, "GET", "/slots?sport=tennis", &student, None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let menus: Vec<(&Value, &Value)> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| (&item["slot_id"], &item["lesson_menu"]))
        .collect();
    assert_eq!(
        menus,
        [
            (&bound["slot_id"], &summary),
            (&plain["slot_id"], &Value::Null)
        ]
    );

    let (status, list) = send(&router, "GET", "/coaches/me/slots", &coach, None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["slots"][0]["slot"]["lesson_menu"], summary);
    assert_eq!(list["slots"][1]["slot"]["lesson_menu"], Value::Null);

    // 予約の作成は今までどおり（予約は枠のメニューを引き継がない）
    let (status, booking) = send(
        &router,
        "POST",
        "/bookings",
        &student,
        Some(json!({ "slot_id": bound["slot_id"], "is_trial": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    assert!(booking.get("lesson_menu_id").is_none(), "{booking}");
}
