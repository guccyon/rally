//! 本番の設定（`ACCESS_TOKEN_SIGNING_KEY` あり。`main` と同じく `AccessTokenBackend::from_vars` で選ぶ）で組み立てた
//! ルーター（`server::router_with_access_tokens`）を通す。Issue #1593。
//!
//! - 本部・コーチ・生徒の操作が、サーバーが発行した署名付きのトークンで通る（生徒は `POST /accounts` が返したもの）
//! - 手で作った `mock.<役割>.<アカウント ID>` のトークンと、署名付きのトークンの役割・アカウント ID を書き換えたものは
//!   どの役割の操作でも 401
//!
//! 鍵のない設定（モック）で今までどおり `mock.` のトークンが通ることは、ほかの `tests/` がそのまま確かめている。

use application::{
    identity::{TokenClaims, TokenIssuer},
    in_memory::RecordingAnalyticsSink,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role};
use infra::MockTokenService;
use serde_json::{Value, json};
use server::{AccessTokenBackend, VideoCallBackend};
use tower::ServiceExt;

/// テストだけの署名の鍵（秘密ではない）。
const TEST_KEY: &str = "server-test-signing-key-0123456789abcdef";

const COACH: AccountId = AccountId::from_u128(42);
const HEADQUARTERS: AccountId = AccountId::from_u128(7);

fn signed_tokens() -> AccessTokenBackend {
    let tokens = AccessTokenBackend::from_vars(|name| {
        (name == AccessTokenBackend::SIGNING_KEY_VAR).then(|| TEST_KEY.to_string())
    })
    .unwrap();
    assert!(tokens.is_signed());
    tokens
}

fn signed_router() -> Router {
    server::router_with_access_tokens(
        application::clock::SystemClock,
        RecordingAnalyticsSink::default(),
        VideoCallBackend::default(),
        signed_tokens(),
    )
}

fn token(issuer: &impl TokenIssuer, account_id: AccountId, role: Role) -> String {
    issuer.issue(TokenClaims { account_id, role }).into_string()
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

fn get(path: &str, token: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn post_json(path: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut request = Request::post(path).header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    request.body(Body::from(body.to_string())).unwrap()
}

async fn register_student(router: &Router) -> String {
    let (status, body) = send(
        router,
        post_json(
            "/accounts",
            None,
            json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["access_token"].as_str().unwrap().to_string()
}

fn slot() -> Value {
    json!({
        "sport": "tennis",
        "starts_at": "2099-01-01T10:00:00Z",
        "ends_at": "2099-01-01T11:00:00Z",
    })
}

#[tokio::test]
async fn headquarters_coach_and_student_operations_pass_with_signed_tokens() {
    let router = signed_router();
    let tokens = signed_tokens();

    // コーチ: 枠を公開する
    let coach = token(&tokens, COACH, Role::Coach);
    assert!(coach.starts_with("v1."), "{coach}");
    let (status, published) = send(&router, post_json("/slots", Some(&coach), slot())).await;
    assert_eq!(status, StatusCode::CREATED, "{published}");
    let (status, body) = send(&router, get("/coaches/me/slots", &coach)).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // 生徒: 登録で署名付きのトークンを受け取り、それで枠を探して予約する
    let student = register_student(&router).await;
    assert!(student.starts_with("v1."), "{student}");
    let (status, me) = send(&router, get("/me", &student)).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["role"], "student", "{me}");
    let (status, body) = send(&router, get("/slots?sport=tennis", &student)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"][0]["slot_id"], published["slot_id"], "{body}");
    let (status, body) = send(
        &router,
        post_json(
            "/bookings",
            Some(&student),
            json!({ "slot_id": published["slot_id"], "is_trial": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    // 本部: 予約枠の需給と生徒の一覧を読む
    let headquarters = token(&tokens, HEADQUARTERS, Role::Headquarters);
    let (status, body) = send(
        &router,
        get("/slot-supply?from=2099-01-01&to=2099-01-31", &headquarters),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send(&router, get("/students", &headquarters)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["students"].as_array().unwrap().len(), 1, "{body}");

    // 役割の体系は変わらない（生徒はコーチの操作を呼べない）
    let (status, _) = send(&router, post_json("/slots", Some(&student), slot())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn handmade_mock_tokens_are_rejected_for_every_role() {
    let router = signed_router();
    // 生徒に見えるコーチの ID から作ったコーチのトークン（#1552 の設計書の「気をつけること」）
    let coach = token(&MockTokenService, COACH, Role::Coach);
    assert!(coach.starts_with("mock."));
    let (status, body) = send(&router, post_json("/slots", Some(&coach), slot())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    let (status, _) = send(&router, get("/coaches/me/slots", &coach)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // 任意の ID の本部のトークン
    let headquarters = token(&MockTokenService, HEADQUARTERS, Role::Headquarters);
    for path in ["/students", "/slot-supply?from=2099-01-01&to=2099-01-31"] {
        let (status, body) = send(&router, get(path, &headquarters)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}: {body}");
    }

    // 登録済みの生徒の ID から作った生徒のトークン
    let student = register_student(&router).await;
    let (_, me) = send(&router, get("/me", &student)).await;
    let student_id = AccountId::from_u128(uuid_value(me["account_id"].as_str().unwrap()));
    let forged = token(&MockTokenService, student_id, Role::Student);
    let (status, _) = send(&router, get("/me", &forged)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn tampered_signed_tokens_are_rejected() {
    let router = signed_router();
    let student = register_student(&router).await;

    // 署名はそのままに、役割を本部に書き換える
    let promoted = student.replacen("v1.student.", "v1.headquarters.", 1);
    assert_ne!(promoted, student);
    let (status, _) = send(&router, get("/students", &promoted)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // 署名を落とす・別の鍵で署名する
    let unsigned = student.rsplit_once('.').unwrap().0.to_string();
    let other_key =
        AccessTokenBackend::from_vars(|_| Some("another-signing-key-0123456789abcdef".to_string()))
            .unwrap();
    let other = token(&other_key, COACH, Role::Coach);
    for forged in [unsigned, other] {
        let (status, _) = send(&router, get("/me", &forged)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{forged}");
    }
}

/// `01234567-89ab-...` の形の UUID の値。
fn uuid_value(text: &str) -> u128 {
    u128::from_str_radix(&text.replace('-', ""), 16).unwrap()
}
