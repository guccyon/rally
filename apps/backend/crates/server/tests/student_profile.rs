//! 生徒の自分のプロフィール（student-app の `GET /me/profile`・`PATCH /me/profile`）を、保存先を Postgres にした組み立て
//! （`server::router_with_postgres`）で通す。変えた表示名が保存されて次の `GET /me/profile` に出ること、空・50 文字を
//! 超えれば 422 で何も変わらないこと、同じパスの `PUT /me/profile`（admin-web のコーチの自己紹介文）が並んで動くこと、
//! コーチ・本部は 403 であることを確かめる。契約の形の細かい検査は interface の
//! `tests/my_profile.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test student_profile -- --include-ignored` で走らせる。

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
async fn student_display_name_is_saved_in_postgres_and_only_students_can_change_it(pool: PgPool) {
    let student = register(
        &pool,
        "taro@example.com",
        Role::Student,
        vec![Sport::Soccer],
    )
    .await;
    let coach = register(&pool, "coach@example.com", Role::Coach, vec![Sport::Soccer]).await;
    let headquarters = register(&pool, "hq@example.com", Role::Headquarters, vec![]).await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());

    let (status, profile) = send(&router, "GET", "/me/profile", &student, None).await;
    assert_eq!(status, StatusCode::OK, "{profile}");
    assert_eq!(profile["display_name"], "テスト");
    assert_eq!(profile["sport"], "soccer");

    let (status, updated) = send(
        &router,
        "PATCH",
        "/me/profile",
        &student,
        Some(json!({ "display_name": "じろう" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["display_name"], "じろう");
    assert_eq!(updated["student_id"], profile["student_id"]);
    assert_eq!(updated["created_at"], profile["created_at"]);
    let (_, profile) = send(&router, "GET", "/me/profile", &student, None).await;
    assert_eq!(profile, updated);

    // 空・50 文字を超えれば 422 で、保存した表示名は変わらない
    for display_name in [String::new(), "あ".repeat(51)] {
        let (status, body) = send(
            &router,
            "PATCH",
            "/me/profile",
            &student,
            Some(json!({ "display_name": display_name })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["code"], "validation_failed");
    }
    let (_, profile) = send(&router, "GET", "/me/profile", &student, None).await;
    assert_eq!(profile["display_name"], "じろう");

    // 同じパスの admin-web のコーチの自己紹介文の変更は、そのまま動く
    let (status, me) = send(
        &router,
        "PUT",
        "/me/profile",
        &coach,
        Some(json!({ "bio": "自己紹介" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["bio"], "自己紹介");

    // コーチ・本部は生徒のプロフィールを見られず、変えられない
    for token in [&coach, &headquarters] {
        let (status, body) = send(&router, "GET", "/me/profile", token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        let (status, body) = send(
            &router,
            "PATCH",
            "/me/profile",
            token,
            Some(json!({ "display_name": "じろう" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        let (_, me) = send(&router, "GET", "/me", token, None).await;
        assert_eq!(me["display_name"], "テスト", "何も変えない");
    }
}
