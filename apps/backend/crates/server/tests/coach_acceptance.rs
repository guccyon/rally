//! 本部によるコーチの受付の停止・再開（admin-web の `POST /coaches/{coach_id}/suspend`・`/resume`）を、保存先を Postgres に
//! した組み立て（`server::router_with_postgres`）で通す。止めたコーチの枠が生徒の検索（`GET /slots`）とコーチを探す一覧
//! （`GET /coaches`）に出ず、予約の作成（`POST /bookings`）が 409 `coach_not_accepting_bookings` になること、止める前に
//! 入った予約は確定できること、本部のコーチの一覧に受付の状態が出ること、再開すると戻ることを確かめる。止めた・再開した記録
//! （`GET /coaches/{coach_id}/acceptance-changes`）が操作した本部のアカウントとともに Postgres に残り、新しい順に読めることも
//! 確かめる。本部のコーチの一覧（`GET /coaches`）を受付の状態と競技で絞れることも確かめる。契約の形の細かい検査は interface の `tests/coach_acceptances.rs`・`tests/coaches.rs`・`tests/bookings.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coach_acceptance -- --include-ignored` で走らせる。

use application::{
    clock::SystemClock,
    identity::{
        RegisterAccount, RegisterAccountCommand, TokenClaims, TokenIssuer, account_id_uuid,
    },
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

/// 役割 `role` のアカウントを登録して（ID, トークン）を返す。契約にコーチ・本部の登録の操作がないので、ユースケースを直に呼ぶ。
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
    (account_id_uuid::format(account.id()), token)
}

/// `coach` が 2099-01-01 の `hour` 時から 1 時間のテニスの枠を公開し、スロット ID を返す。
async fn publish(router: &Router, coach: &str, hour: u32) -> String {
    let (status, slot) = send(
        router,
        "POST",
        "/slots",
        coach,
        Some(json!({
            "sport": "tennis",
            "starts_at": format!("2099-01-01T{hour}:00:00Z"),
            "ends_at": format!("2099-01-01T{}:00:00Z", hour + 1),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    slot["slot_id"].as_str().unwrap().to_string()
}

fn ids(body: &Value, list: &str, id: &str) -> Vec<String> {
    body[list]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item[id].as_str().unwrap().to_string())
        .collect()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn suspended_coach_is_hidden_from_students_and_takes_no_new_booking_until_resumed(
    pool: PgPool,
) {
    let (coach_id, coach) =
        register(&pool, "coach@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let (_, headquarters) = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let (_, student) = register(
        &pool,
        "taro@example.com",
        Role::Student,
        vec![Sport::Tennis],
    )
    .await;
    let (_, other_student) = register(
        &pool,
        "jiro@example.com",
        Role::Student,
        vec![Sport::Tennis],
    )
    .await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());
    let booked = publish(&router, &coach, 10).await;
    let open = publish(&router, &coach, 12).await;
    let (status, booking) = send(
        &router,
        "POST",
        "/bookings",
        &student,
        Some(json!({ "slot_id": booked, "is_trial": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");

    let (status, body) = send(
        &router,
        "POST",
        &format!("/coaches/{coach_id}/suspend"),
        &headquarters,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["acceptance_status"], "suspended");

    // 本部の一覧には状態つきで残り、生徒の検索とコーチを探す一覧からは消える
    let (_, coaches) = send(&router, "GET", "/coaches", &headquarters, None).await;
    assert_eq!(
        coaches["coaches"],
        json!([{ "coach_id": coach_id, "display_name": "テスト", "acceptance_status": "suspended" }])
    );
    let (_, slots) = send(&router, "GET", "/slots?sport=tennis", &other_student, None).await;
    assert!(ids(&slots, "items", "slot_id").is_empty(), "{slots}");
    let (_, coaches) = send(&router, "GET", "/coaches", &other_student, None).await;
    assert!(ids(&coaches, "coaches", "coach_id").is_empty(), "{coaches}");

    // 新しい予約は 409、止める前に入った予約は確定できる
    let (status, body) = send(
        &router,
        "POST",
        "/bookings",
        &other_student,
        Some(json!({ "slot_id": open, "is_trial": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "coach_not_accepting_bookings");
    let booking_id = booking["booking_id"].as_str().unwrap();
    let (status, body) = send(
        &router,
        "POST",
        &format!("/bookings/{booking_id}/confirm"),
        &student,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // 再開すると戻る
    let (status, body) = send(
        &router,
        "POST",
        &format!("/coaches/{coach_id}/resume"),
        &headquarters,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["acceptance_status"], "accepting");
    let (_, slots) = send(&router, "GET", "/slots?sport=tennis", &other_student, None).await;
    assert_eq!(ids(&slots, "items", "slot_id"), [open.as_str()], "{slots}");
    let (status, body) = send(
        &router,
        "POST",
        "/bookings",
        &other_student,
        Some(json!({ "slot_id": open, "is_trial": false })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    // コーチは止められない（本部だけ）
    let (status, _) = send(
        &router,
        "POST",
        &format!("/coaches/{coach_id}/suspend"),
        &coach,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_who_suspended_and_resumed_a_coach_newest_first(pool: PgPool) {
    let (coach_id, coach) =
        register(&pool, "coach@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let (hq_id, headquarters) = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let (other_hq_id, other_headquarters) =
        register(&pool, "hq2@example.com", Role::Headquarters, vec![]).await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());
    let path = format!("/coaches/{coach_id}/acceptance-changes");

    let (status, body) = send(&router, "GET", &path, &headquarters, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "coach_id": coach_id, "changes": [] }));

    for (action, token) in [("suspend", &headquarters), ("resume", &other_headquarters)] {
        let (status, body) = send(
            &router,
            "POST",
            &format!("/coaches/{coach_id}/{action}"),
            token,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{action}: {body}");
    }

    let (status, body) = send(&router, "GET", &path, &other_headquarters, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let changes = body["changes"].as_array().unwrap();
    let summary: Vec<_> = changes
        .iter()
        .map(|c| (c["acceptance_status"].clone(), c["changed_by"].clone()))
        .collect();
    assert_eq!(
        summary,
        [
            (json!("accepting"), json!(other_hq_id)),
            (json!("suspended"), json!(hq_id)),
        ]
    );
    // RFC 3339 の UTC で揃うので、文字列の順が時刻の順
    assert!(changes[0]["changed_at"].as_str() >= changes[1]["changed_at"].as_str());

    // 本部だけ。コーチでない ID は 404
    let (status, _) = send(&router, "GET", &path, &coach, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = send(
        &router,
        "GET",
        &format!("/coaches/{hq_id}/acceptance-changes"),
        &headquarters,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_filters_coaches_by_acceptance_status_and_sport(pool: PgPool) {
    let (tennis_id, _) = register(
        &pool,
        "tennis@example.com",
        Role::Coach,
        vec![Sport::Tennis],
    )
    .await;
    let (soccer_id, _) = register(
        &pool,
        "soccer@example.com",
        Role::Coach,
        vec![Sport::Soccer, Sport::Golf],
    )
    .await;
    let (_, headquarters) = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let (_, student) = register(
        &pool,
        "taro@example.com",
        Role::Student,
        vec![Sport::Tennis],
    )
    .await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());
    let (status, body) = send(
        &router,
        "POST",
        &format!("/coaches/{tennis_id}/suspend"),
        &headquarters,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = send(&router, "GET", "/coaches", &headquarters, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut all = ids(&body, "coaches", "coach_id");
    all.sort();
    let mut expected = vec![tennis_id.clone(), soccer_id.clone()];
    expected.sort();
    assert_eq!(all, expected, "条件がなければ今までどおりすべて");

    for (query, expected) in [
        ("?acceptance_status=suspended", vec![tennis_id.as_str()]),
        ("?acceptance_status=accepting", vec![soccer_id.as_str()]),
        ("?sport=golf", vec![soccer_id.as_str()]),
        (
            "?sport=tennis&acceptance_status=suspended",
            vec![tennis_id.as_str()],
        ),
        ("?sport=tennis&acceptance_status=accepting", vec![]),
    ] {
        let (status, body) = send(
            &router,
            "GET",
            &format!("/coaches{query}"),
            &headquarters,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(ids(&body, "coaches", "coach_id"), expected, "{query}");
    }
    let (_, body) = send(
        &router,
        "GET",
        "/coaches?acceptance_status=suspended",
        &headquarters,
        None,
    )
    .await;
    assert_eq!(body["coaches"][0]["acceptance_status"], "suspended");

    let (status, body) = send(
        &router,
        "GET",
        "/coaches?acceptance_status=paused",
        &headquarters,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    // 生徒には今までどおり（受付の状態の条件は見ない。止めたテニスのコーチは出ない）
    let (status, body) = send(
        &router,
        "GET",
        "/coaches?acceptance_status=suspended",
        &student,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(ids(&body, "coaches", "coach_id").is_empty(), "{body}");
}
