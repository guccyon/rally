//! 生徒のログイン（student-app の `POST /login-codes`・`POST /sessions`。ADR 0012）を組み立てたサーバーで通す。登録した生徒が
//! メールで届いたコードでログインし直し、受け取ったトークンで本人の照会（`GET /me/profile`）が通ること、登録のない
//! メールアドレスも同じ 202 になることを、インメモリと Postgres の両方の組み立てで確かめる。契約の形の細かい検査は interface の
//! `tests/it/student_login.rs`。
//!
//! Postgres を使うものは既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p server --test it student_login:: -- --include-ignored` で走らせる。

use application::{clock::SystemClock, in_memory::RecordingAnalyticsSink};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use infra::MockLoginCodeMailer;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
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

/// 生徒を登録して、その `student_id` を返す。
async fn register(router: &Router, email: &str) -> Value {
    let (status, body) = send(
        router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": email, "display_name": "たろう", "sport": "tennis" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["student"]["student_id"].clone()
}

async fn request_code(router: &Router, email: &str) -> (StatusCode, Value) {
    send(
        router,
        "POST",
        "/login-codes",
        None,
        Some(json!({ "email": email })),
    )
    .await
}

async fn create_session(router: &Router, email: &str, code: &str) -> (StatusCode, Value) {
    send(
        router,
        "POST",
        "/sessions",
        None,
        Some(json!({ "email": email, "code": code })),
    )
    .await
}

/// ログインで受け取ったトークンで、本人の照会が通り同じ生徒が返る。
async fn assert_logged_in_as(router: &Router, session: &Value, student_id: &Value) {
    assert_eq!(&session["student"]["student_id"], student_id);
    let token = session["access_token"].as_str().unwrap();
    let (status, profile) = send(router, "GET", "/me/profile", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{profile}");
    assert_eq!(&profile["student_id"], student_id);
}

#[tokio::test]
async fn a_registered_student_logs_in_again_with_the_mailed_code() {
    let mailer = MockLoginCodeMailer::new();
    let router = server::router_with_login_code_mailer(
        SystemClock,
        RecordingAnalyticsSink::default(),
        mailer.clone(),
    );
    let student_id = register(&router, "taro@example.com").await;

    // 登録し直そうとすると、409 でログインへ案内される
    let (status, conflict) = send(
        &router,
        "POST",
        "/accounts",
        None,
        Some(json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        conflict["login_guidance"]["request_login_code_path"],
        "/login-codes"
    );

    let (registered_status, registered) = request_code(&router, "taro@example.com").await;
    let (unknown_status, unknown) = request_code(&router, "nobody@example.com").await;
    assert_eq!(registered_status, StatusCode::ACCEPTED);
    assert_eq!(unknown_status, StatusCode::ACCEPTED);
    assert_eq!(
        registered["expires_in_seconds"],
        unknown["expires_in_seconds"]
    );
    assert_eq!(
        registered["resend_after_seconds"],
        unknown["resend_after_seconds"]
    );

    let sent = mailer.sent();
    assert_eq!(sent.len(), 1, "登録のないメールアドレスには送らない");
    let code = sent[0].1.as_str().to_string();

    let (status, session) = create_session(&router, "taro@example.com", &code).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_logged_in_as(&router, &session, &student_id).await;

    let (status, again) = create_session(&router, "taro@example.com", &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(again["code"], "login_code_invalid");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn login_codes_are_kept_in_postgres(pool: PgPool) {
    let router =
        server::router_with_postgres(pool.clone(), SystemClock, RecordingAnalyticsSink::default());
    let student_id = register(&router, "taro@example.com").await;

    for email in ["Taro@Example.com", "nobody@example.com"] {
        let (status, body) = request_code(&router, email).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{email}: {body}");
    }
    // 60 秒たつまでは送り直せない（登録の有無によらない）
    for email in ["taro@example.com", "NOBODY@example.com"] {
        let (status, body) = request_code(&router, email).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{email}");
        assert_eq!(body["code"], "too_many_requests");
    }

    let rows: Vec<(String, Option<sqlx::types::Uuid>, String)> = sqlx::query_as(
        "SELECT email_key, account_id, code FROM identity.login_codes ORDER BY email_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "nobody@example.com");
    assert_eq!(rows[0].1, None);
    assert_eq!(rows[1].0, "taro@example.com");
    assert_eq!(
        rows[1].1.map(|id| id.to_string()),
        student_id.as_str().map(str::to_string)
    );
    let code = rows[1].2.clone();

    let wrong = if code == "000000" { "111111" } else { "000000" };
    let (status, body) = create_session(&router, "taro@example.com", wrong).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "login_code_invalid");
    let failed: i32 = sqlx::query_scalar(
        "SELECT failed_attempts FROM identity.login_codes WHERE email_key = 'taro@example.com'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failed, 1);

    let (status, session) = create_session(&router, "taro@example.com", &code).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_logged_in_as(&router, &session, &student_id).await;

    let (status, body) = create_session(&router, "taro@example.com", &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "login_code_invalid");
}
