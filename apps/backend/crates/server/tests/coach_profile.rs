//! コーチの自己紹介文（admin-web の `PUT /me/profile` と `GET /me` の `bio`）を、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。書いた自己紹介文が保存されて次の `GET /me` に出ること、400 文字を超えれば
//! 422 で何も変わらないこと、本部・生徒は 403 であることを確かめる。書いた自己紹介文が、生徒が見るコーチの紹介
//! （student-app の `GET /coaches/{coach_id}`）にも出ることを確かめる。契約の形の細かい検査は interface の `tests/me.rs` と
//! `tests/coach_profiles.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test coach_profile -- --include-ignored` で走らせる。

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

/// 役割 `role` のアカウントを登録してトークンを返す。契約にコーチ・本部の登録の操作がないので、ユースケースを直に呼ぶ。
async fn register(pool: &PgPool, email: &str, role: Role, sports: Vec<Sport>) -> String {
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
    MockTokenService
        .issue(TokenClaims {
            account_id: account.id(),
            role,
        })
        .into_string()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn coach_bio_is_saved_in_postgres_and_only_coaches_can_write_it(pool: PgPool) {
    let coach = register(&pool, "coach@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let headquarters = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let student = register(
        &pool,
        "taro@example.com",
        Role::Student,
        vec![Sport::Soccer],
    )
    .await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());

    let (status, me) = send(&router, "GET", "/me", &coach, None).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["bio"], "", "書いていなければ空");

    let (status, updated) = send(
        &router,
        "PUT",
        "/me/profile",
        &coach,
        Some(json!({ "bio": "テニス歴 20 年。初心者歓迎" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["bio"], "テニス歴 20 年。初心者歓迎");
    let (_, me) = send(&router, "GET", "/me", &coach, None).await;
    assert_eq!(me, updated);

    // 400 文字を超えれば 422 で、保存した文は変わらない
    let (status, body) = send(
        &router,
        "PUT",
        "/me/profile",
        &coach,
        Some(json!({ "bio": "あ".repeat(401) })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
    let (_, me) = send(&router, "GET", "/me", &coach, None).await;
    assert_eq!(me["bio"], "テニス歴 20 年。初心者歓迎");

    // 生徒はコーチの紹介で、書いた自己紹介文を読める
    let coach_id = me["account_id"].as_str().unwrap();
    let (status, profile) = send(
        &router,
        "GET",
        &format!("/coaches/{coach_id}"),
        &student,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{profile}");
    assert_eq!(
        profile,
        json!({
            "coach_id": coach_id,
            "display_name": "テスト",
            "bio": "テニス歴 20 年。初心者歓迎",
            "sports": ["tennis"],
            "is_favorite": false
        })
    );

    // 本部・生徒は書けず、GET /me に bio は載らない
    for token in [&headquarters, &student] {
        let (status, body) = send(
            &router,
            "PUT",
            "/me/profile",
            token,
            Some(json!({ "bio": "自己紹介" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(body["code"], "forbidden");
        let (status, me) = send(&router, "GET", "/me", token, None).await;
        assert_eq!(status, StatusCode::OK, "{me}");
        assert!(me.get("bio").is_none(), "{me}");
    }
}
