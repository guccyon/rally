//! 生徒のお気に入りのコーチ（student-app の `PUT`・`DELETE /favorite-coaches/{coach_id}` と `GET /favorite-coaches`）を、
//! 保存先を Postgres にした組み立て（`server::router_with_postgres`）で通す。入れたコーチが一覧に 1 件だけ出ること、
//! コーチを探す一覧（`GET /coaches`）とコーチの紹介（`GET /coaches/{coach_id}`）の `is_favorite` に出ること、外すと
//! 消えることを確かめる。契約の形の細かい検査は interface の `tests/favorite_coaches.rs`。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test favorite_coaches -- --include-ignored` で走らせる。

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
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

async fn send(router: &Router, method: &str, path: &str, token: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// 役割 `role` のアカウントを登録して、ID とトークンを返す。契約にコーチの登録の操作がないので、ユースケースを直に呼ぶ。
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

fn favorite_ids(body: &Value) -> Vec<&str> {
    body["favorite_coaches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["coach_id"].as_str().unwrap())
        .collect()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_student_favorites_a_coach_in_postgres(pool: PgPool) {
    let (coach_id, _) =
        register(&pool, "coach@example.com", Role::Coach, vec![Sport::Tennis]).await;
    let (other_coach_id, _) = register(
        &pool,
        "coach2@example.com",
        Role::Coach,
        vec![Sport::Tennis],
    )
    .await;
    let (_, student) = register(
        &pool,
        "taro@example.com",
        Role::Student,
        vec![Sport::Tennis],
    )
    .await;
    let router = server::router_with_postgres(pool, SystemClock, RecordingAnalyticsSink::default());

    for _ in 0..2 {
        let (status, body) = send(
            &router,
            "PUT",
            &format!("/favorite-coaches/{coach_id}"),
            &student,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            favorite_ids(&body),
            [coach_id.as_str()],
            "2 回入れても 1 件"
        );
    }
    let (status, body) = send(&router, "GET", "/favorite-coaches", &student).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(favorite_ids(&body), [coach_id.as_str()]);
    assert_eq!(body["favorite_coaches"][0]["sports"][0], "tennis");

    let (status, body) = send(&router, "GET", "/coaches", &student).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let favorites: Vec<(&str, bool)> = body["coaches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["coach_id"].as_str().unwrap(),
                c["is_favorite"].as_bool().unwrap(),
            )
        })
        .filter(|(_, is_favorite)| *is_favorite)
        .collect();
    assert_eq!(favorites, [(coach_id.as_str(), true)]);
    let (_, profile) = send(&router, "GET", &format!("/coaches/{coach_id}"), &student).await;
    assert_eq!(profile["is_favorite"], true);
    let (_, profile) = send(
        &router,
        "GET",
        &format!("/coaches/{other_coach_id}"),
        &student,
    )
    .await;
    assert_eq!(profile["is_favorite"], false);

    let (status, body) = send(
        &router,
        "DELETE",
        &format!("/favorite-coaches/{coach_id}"),
        &student,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(favorite_ids(&body).is_empty());
    let (_, profile) = send(&router, "GET", &format!("/coaches/{coach_id}"), &student).await;
    assert_eq!(profile["is_favorite"], false);
}
