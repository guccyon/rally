//! `GET /slot-supply` を組み立て済みのルーター（`server::router()`）で通す。トークンは infra のモック
//! （`MockTokenService`）で発行する。契約の形・数え方の検査は interface の `tests/slot_supply.rs`、
//! 合成シードでの判定は seed の `tests/slot_supply.rs`。日付（`from`・`to`）で期間を指定して Postgres の枠と予約を数える
//! ことは、保存先を Postgres にした組み立てで確かめる。

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
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
};
use infra::{MockTokenService, PgAccountRepository};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

async fn get_slot_supply(role: Role) -> (StatusCode, Value) {
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role,
        })
        .into_string();
    let request = Request::get("/slot-supply")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = server::router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn headquarters_reads_the_supply_of_the_six_sports() {
    let (status, body) = get_slot_supply(Role::Headquarters).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sports: Vec<&str> = body["sports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["sport"].as_str().unwrap())
        .collect();
    assert_eq!(
        sports,
        [
            "tennis",
            "golf",
            "baseball",
            "soccer",
            "basketball",
            "swimming"
        ]
    );
}

#[tokio::test]
async fn coaches_and_students_may_not_read_the_supply() {
    for role in [Role::Coach, Role::Student] {
        let (status, body) = get_slot_supply(role).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{role:?}: {body}");
    }
}

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

/// アカウントを登録してトークンを返す。契約にコーチ・本部の登録の操作がないので、ユースケースを直に呼ぶ。
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
        sports: if role == Role::Headquarters {
            vec![]
        } else {
            vec![Sport::Tennis]
        },
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

/// 保存先を Postgres にした組み立て（`server::router_with_postgres`）で、来週の日付（日本時間）を指定して需給を読む。
/// Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
/// `cargo test -p server --test slot_supply -- --include-ignored` で走らせる。
#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_reads_the_supply_of_next_week_from_postgres(pool: PgPool) {
    let coach = register(&pool, "tanaka@example.com", Role::Coach).await;
    let student = register(&pool, "taro@example.com", Role::Student).await;
    let hq = register(&pool, "hq@example.com", Role::Headquarters).await;
    let router = server::router_with_postgres(
        pool,
        FixedClock(SystemTime::UNIX_EPOCH + NOW),
        RecordingAnalyticsSink::default(),
    );

    // 来週（日本時間の 11 月 2 日から 8 日）の始まりちょうど（2 日 0 時）と終わりの日の夜（8 日 22 時）に 1 つずつ、
    // その直後（9 日 0 時）に 1 つ
    let mut slot_ids = Vec::new();
    for (starts_at, ends_at) in [
        ("2026-11-01T15:00:00Z", "2026-11-01T16:00:00Z"),
        ("2026-11-08T13:00:00Z", "2026-11-08T14:00:00Z"),
        ("2026-11-08T15:00:00Z", "2026-11-08T16:00:00Z"),
    ] {
        let (status, slot) = send(
            &router,
            post_json(
                "/slots",
                &coach,
                json!({ "sport": "tennis", "starts_at": starts_at, "ends_at": ends_at }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        slot_ids.push(slot["slot_id"].clone());
    }
    // 期間の外の枠への予約は数えない
    for (slot_id, is_trial) in [(&slot_ids[0], true), (&slot_ids[2], false)] {
        let (status, booking) = send(
            &router,
            post_json(
                "/bookings",
                &student,
                json!({ "slot_id": slot_id, "is_trial": is_trial }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
    }

    let (status, body) = send(
        &router,
        get("/slot-supply?from=2026-11-02&to=2026-11-08", &hq),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["from"], "2026-11-02");
    assert_eq!(body["to"], "2026-11-08");
    assert_eq!(body["window_start"], "2026-11-01T15:00:00Z");
    assert_eq!(body["window_end"], "2026-11-08T15:00:00Z");
    // 公開した 2 枠に 1 人（50%）
    assert_eq!(
        body["sports"][0],
        json!({ "sport": "tennis", "status": "sufficient", "published_slots": 2, "booking_students": 1 })
    );

    // 両方省けば今までどおり、今までの 30 日（来週の枠は入らない）
    let (status, body) = send(&router, get("/slot-supply", &hq)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["window_end"], "2026-10-26T00:00:00Z");
    assert!(body.get("from").is_none(), "{body}");
    assert_eq!(body["sports"][0]["published_slots"], 0, "{body}");

    // 期間の違反は 422
    let (status, body) = send(
        &router,
        get("/slot-supply?from=2026-11-02&to=2026-12-03", &hq),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
}
