//! 本部が見る、プロフィールに足りない項目のあるコーチの一覧（admin-web の `GET /coaches/incomplete-profiles`）を、保存先を
//! Postgres にした組み立て（`server::router_with_postgres`）で通す。自己紹介文を書いていない・空白だけのコーチが載り、
//! 書き終えたコーチ（`PUT /me/profile`）は載らず、消せばまた載ること、`sport` で提供スポーツの合うコーチだけに絞れる
//! （保存先の提供スポーツの読み方まで通す）こと、コーチ・生徒は 403 であることを確かめる。
//! 契約の形の細かい検査は interface の `tests/incomplete_coach_profiles.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test incomplete_coach_profiles -- --include-ignored` で走らせる。

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

/// 2026-10-01T00:00:00Z から `hour` 時間後。
fn at(hour: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_812_800 + hour * 3600)
}

/// 役割 `role` のアカウントを `registered_hour` 時に登録して、トークンを返す。契約にコーチ・本部の登録の
/// 操作がないので、ユースケースを直に呼ぶ。
async fn register(
    pool: &PgPool,
    email: &str,
    role: Role,
    sports: Vec<Sport>,
    registered_hour: u64,
) -> String {
    let account = RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        FixedClock(at(registered_hour)),
    )
    .execute(RegisterAccountCommand {
        email: email.to_string(),
        display_name: email.split('@').next().unwrap().to_string(),
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

/// 一覧に載ったコーチの (表示名, missing_fields, updated_at, registered_at)。
fn listed(body: &Value) -> Vec<Value> {
    body["coaches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            json!([
                c["display_name"],
                c["missing_fields"],
                c["updated_at"],
                c["registered_at"]
            ])
        })
        .collect()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn headquarters_finds_coaches_whose_bio_is_missing_in_postgres(pool: PgPool) {
    let headquarters = register(&pool, "hq@example.com", Role::Headquarters, vec![], 0).await;
    let student = register(
        &pool,
        "taro@example.com",
        Role::Student,
        vec![Sport::Soccer],
        0,
    )
    .await;
    let written = register(
        &pool,
        "written@example.com",
        Role::Coach,
        vec![Sport::Tennis],
        1,
    )
    .await;
    let blank = register(
        &pool,
        "blank@example.com",
        Role::Coach,
        vec![Sport::Golf],
        3,
    )
    .await;
    let fresh = register(
        &pool,
        "fresh@example.com",
        Role::Coach,
        vec![Sport::Baseball],
        2,
    )
    .await;
    let router =
        server::router_with_postgres(pool.clone(), SystemClock, RecordingAnalyticsSink::default());

    // 書き終えたコーチ・空白だけのコーチ
    for (token, bio) in [(&written, "テニス歴 20 年"), (&blank, "\u{3000}\n")] {
        let (status, body) = send(
            &router,
            "PUT",
            "/me/profile",
            token,
            Some(json!({ "bio": bio })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    let (status, body) = send(
        &router,
        "GET",
        "/coaches/incomplete-profiles",
        &headquarters,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        listed(&body),
        [
            json!(["fresh", ["bio"], null, "2026-10-01T02:00:00Z"]),
            json!(["blank", ["bio"], null, "2026-10-01T03:00:00Z"]),
        ],
        "登録の古い順。書き終えたコーチ・本部・生徒は載らない"
    );

    // 自己紹介文を消せばまた載る
    let (status, _) = send(
        &router,
        "PUT",
        "/me/profile",
        &written,
        Some(json!({ "bio": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = send(
        &router,
        "GET",
        "/coaches/incomplete-profiles",
        &headquarters,
        None,
    )
    .await;
    let names: Vec<&str> = body["coaches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["display_name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["written", "fresh", "blank"]);

    // 競技で絞る。提供スポーツが複数のコーチも、どれかが合えば載る
    let multi = register(
        &pool,
        "multi@example.com",
        Role::Coach,
        vec![Sport::Tennis, Sport::Golf],
        4,
    )
    .await;
    let names_in = |body: &Value| -> Vec<String> {
        body["coaches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["display_name"].as_str().unwrap().to_string())
            .collect()
    };
    for (query, expected) in [
        ("golf", &["blank", "multi"][..]),
        ("tennis", &["written", "multi"][..]),
        ("baseball", &["fresh"][..]),
        ("swimming", &[][..]),
    ] {
        let (status, body) = send(
            &router,
            "GET",
            &format!("/coaches/incomplete-profiles?sport={query}"),
            &headquarters,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(names_in(&body), expected, "{query}");
    }
    let (status, body) = send(
        &router,
        "GET",
        "/coaches/incomplete-profiles?sport=curling",
        &headquarters,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");

    // コーチ・生徒は 403
    for token in [&written, &fresh, &multi, &student] {
        let (status, body) =
            send(&router, "GET", "/coaches/incomplete-profiles", token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(body["code"], "forbidden");
    }
}
