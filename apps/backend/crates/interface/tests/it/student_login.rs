//! `POST /login-codes`・`POST /sessions`（student-app の `requestLoginCode`・`createSession`。ADR 0012）の統合テスト。
//! レスポンスの形を student-app の OpenAPI（specs/contracts/api/student-app/openapi.yaml）のスキーマで検査する。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        AccessToken, AccountRepository, CreateSession, InvalidToken, RequestLoginCode, TokenClaims,
        TokenIssuer, TokenVerifier, account_id_uuid,
    },
    in_memory::{InMemoryAccountRepository, ManualClock, RecordingLoginCodeSender},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    profile::DisplayName,
};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::support;

/// 2026-10-26T00:00:00Z
const NOW: Duration = Duration::from_secs(1_792_972_800);
const STUDENT: AccountId = AccountId::from_u128(1);

fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW + Duration::from_secs(secs)
}

/// 発行した内容が読める形のトークン（interface は infra のモックに依存できないので、テスト用に置く）。
struct FakeTokens;

impl TokenIssuer for FakeTokens {
    fn issue(&self, claims: TokenClaims) -> AccessToken {
        AccessToken::new(format!(
            "fake:{:?}:{}",
            claims.role,
            account_id_uuid::format(claims.account_id)
        ))
    }
}

impl TokenVerifier for FakeTokens {
    fn verify(&self, _token: &str) -> Result<TokenClaims, InvalidToken> {
        Err(InvalidToken)
    }
}

struct App {
    router: Router,
    sender: RecordingLoginCodeSender,
    clock: ManualClock,
}

async fn app() -> App {
    let accounts = InMemoryAccountRepository::default();
    for (id, email, role) in [
        (1, "taro@example.com", Role::Student),
        (2, "coach@example.com", Role::Coach),
    ] {
        accounts
            .insert(
                Account::new(
                    AccountId::from_u128(id),
                    Email::new(email).unwrap(),
                    DisplayName::new("たろう").unwrap(),
                    role,
                    [Sport::Tennis],
                    at(0),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    let sender = RecordingLoginCodeSender::default();
    let clock = ManualClock::new(at(0));
    let router = interface::student_login_router(
        RequestLoginCode::new(accounts.clone(), sender.clone(), clock.clone()),
        CreateSession::new(accounts, clock.clone()),
        FakeTokens,
    );
    App {
        router,
        sender,
        clock,
    }
}

async fn post(router: &Router, path: &str, body: Value) -> (StatusCode, HeaderMap, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    support::Contract::load("student-app").assert_response("post", path, status, &body);
    (status, headers, body)
}

async fn request_code(app: &App, email: &str) -> (StatusCode, HeaderMap, Value) {
    post(&app.router, "/login-codes", json!({ "email": email })).await
}

async fn create_session(app: &App, email: &str, code: &str) -> (StatusCode, HeaderMap, Value) {
    post(
        &app.router,
        "/sessions",
        json!({ "email": email, "code": code }),
    )
    .await
}

fn sent_code(app: &App, email: &str) -> String {
    app.sender
        .last_code_to(email)
        .expect("コードを送っていない")
        .as_str()
        .to_string()
}

fn other_than(code: &str) -> &'static str {
    if code == "000000" { "111111" } else { "000000" }
}

#[tokio::test]
async fn request_login_code_is_the_same_202_for_registered_and_unregistered_emails() {
    let app = app().await;
    let mut bodies = Vec::new();
    for email in [
        "taro@example.com",
        "nobody@example.com",
        "coach@example.com",
    ] {
        let (status, _, body) = request_code(&app, email).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{email}: {body}");
        assert_eq!(body["email"], email);
        bodies.push(json!({
            "expires_in_seconds": body["expires_in_seconds"],
            "resend_after_seconds": body["resend_after_seconds"],
        }));
    }
    for body in bodies {
        assert_eq!(
            body,
            json!({ "expires_in_seconds": 600, "resend_after_seconds": 60 })
        );
    }
    // 送るのは生徒のアカウントのメールアドレスにだけ
    let sent = app.sender.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0.as_str(), "taro@example.com");
}

#[tokio::test]
async fn the_sent_code_returns_the_student_and_a_token_once() {
    let app = app().await;
    request_code(&app, "taro@example.com").await;
    let code = sent_code(&app, "taro@example.com");

    let (status, _, body) = create_session(&app, "Taro@Example.com", &code).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["student"],
        json!({
            "student_id": account_id_uuid::format(STUDENT),
            "display_name": "たろう",
            "sport": "tennis",
            "created_at": "2026-10-26T00:00:00Z",
        })
    );
    assert_eq!(
        body["access_token"],
        format!("fake:Student:{}", account_id_uuid::format(STUDENT))
    );

    let (status, _, body) = create_session(&app, "taro@example.com", &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "login_code_invalid");
}

#[tokio::test]
async fn wrong_code_unsent_code_and_non_students_are_401_login_code_invalid() {
    let app = app().await;
    let (status, _, body) = create_session(&app, "taro@example.com", "123456").await;
    assert_eq!(
        (status, &body["code"]),
        (StatusCode::UNAUTHORIZED, &json!("login_code_invalid"))
    );

    request_code(&app, "taro@example.com").await;
    let code = sent_code(&app, "taro@example.com");
    let (status, _, body) = create_session(&app, "taro@example.com", other_than(&code)).await;
    assert_eq!(
        (status, &body["code"]),
        (StatusCode::UNAUTHORIZED, &json!("login_code_invalid"))
    );

    for email in ["nobody@example.com", "coach@example.com"] {
        request_code(&app, email).await;
        let (status, _, body) = create_session(&app, email, "123456").await;
        assert_eq!(
            (status, &body["code"]),
            (StatusCode::UNAUTHORIZED, &json!("login_code_invalid")),
            "{email}"
        );
    }
}

#[tokio::test]
async fn ten_minutes_after_sending_the_code_is_401_login_code_expired() {
    let app = app().await;
    request_code(&app, "taro@example.com").await;
    let code = sent_code(&app, "taro@example.com");

    app.clock.set(at(600));
    let (status, _, body) = create_session(&app, "taro@example.com", &code).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "login_code_expired");
}

#[tokio::test]
async fn resending_within_sixty_seconds_is_429_with_retry_after() {
    let app = app().await;
    for email in ["taro@example.com", "nobody@example.com"] {
        request_code(&app, email).await;
    }

    app.clock.set(at(45));
    for email in ["taro@example.com", "nobody@example.com"] {
        let (status, headers, body) = request_code(&app, email).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{email}");
        assert_eq!(body["code"], "too_many_requests");
        assert_eq!(headers[header::RETRY_AFTER], "15");
    }

    app.clock.set(at(60));
    let (status, _, _) = request_code(&app, "taro@example.com").await;
    assert_eq!(status, StatusCode::ACCEPTED);
}

#[tokio::test]
async fn five_mistakes_make_the_code_429() {
    let app = app().await;
    request_code(&app, "taro@example.com").await;
    let code = sent_code(&app, "taro@example.com");
    for _ in 0..5 {
        let (status, _, _) = create_session(&app, "taro@example.com", other_than(&code)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    app.clock.set(at(20));
    let (status, headers, body) = create_session(&app, "taro@example.com", &code).await;

    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "too_many_requests");
    assert_eq!(headers[header::RETRY_AFTER], "40");
}

#[tokio::test]
async fn malformed_input_is_422_validation_failed() {
    let app = app().await;
    for (path, body) in [
        ("/login-codes", json!({ "email": "taro" })),
        ("/login-codes", json!({ "email": "" })),
        ("/login-codes", json!({})),
        ("/sessions", json!({ "email": "taro", "code": "123456" })),
        (
            "/sessions",
            json!({ "email": "taro@example.com", "code": "12345" }),
        ),
        (
            "/sessions",
            json!({ "email": "taro@example.com", "code": "abcdef" }),
        ),
        ("/sessions", json!({ "email": "taro@example.com" })),
    ] {
        let (status, _, response) = post(&app.router, path, body.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path} {body}");
        assert_eq!(response["code"], "validation_failed");
    }
    assert!(app.sender.sent().is_empty());
}
