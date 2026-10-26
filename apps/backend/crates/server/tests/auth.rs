//! 認証と役割による認可の統合テスト。トークンは infra のモック（`MockTokenService`）で発行・検証する。
//!
//! Gen1 の契約にはまだ認証必須のエンドポイントの実装がないので、`AuthenticatedAccount` と
//! `require_role` を使う検査用のルートをこのテストで組み立てる。

use std::sync::Arc;

use application::identity::{TokenClaims, TokenIssuer};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    routing::get,
};
use domain::identity::{AccountId, Role};
use infra::MockTokenService;
use interface::{AuthenticatedAccount, auth::TokenVerification, require_role};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn whoami(account: AuthenticatedAccount) -> Json<Value> {
    Json(json!({
        "account_id": account.account_id.to_string(),
        "role": account.role.as_str(),
    }))
}

/// `/any` は認証だけ、`/coach-only` はコーチと本部だけ（admin-web のコーチの操作の想定）。
fn protected() -> Router {
    Router::new()
        .route("/coach-only", get(whoami))
        .route_layer(require_role(&[Role::Coach, Role::Headquarters]))
        .route("/any", get(whoami))
        .layer(TokenVerification::layer(Arc::new(MockTokenService)))
}

async fn send(router: Router, request: Request<Body>) -> (StatusCode, Option<String>, Value) {
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let www_authenticate = response
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (
        status,
        www_authenticate,
        serde_json::from_slice(&bytes).unwrap(),
    )
}

async fn get_with(path: &str, authorization: Option<&str>) -> (StatusCode, Option<String>, Value) {
    let mut request = Request::get(path);
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, value);
    }
    send(protected(), request.body(Body::empty()).unwrap()).await
}

fn token(role: Role) -> String {
    MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role,
        })
        .into_string()
}

/// `POST /accounts` で生徒を登録し、(student_id, access_token) を返す。
async fn register_student() -> (String, String) {
    let request = Request::post("/accounts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "email": "taro@example.com", "display_name": "たろう", "sport": "tennis" })
                .to_string(),
        ))
        .unwrap();
    let (status, _, body) = send(server::router(), request).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (
        body["student"]["student_id"].as_str().unwrap().to_string(),
        body["access_token"].as_str().unwrap().to_string(),
    )
}

#[tokio::test]
async fn missing_or_invalid_token_is_401_unauthorized() {
    let cases = [
        ("Authorization なし", None),
        ("Bearer でない", Some("Basic dGFybzpwYXNz".to_string())),
        ("トークンが空", Some("Bearer ".to_string())),
        ("形が不正", Some("Bearer not-a-token".to_string())),
        (
            "未知の役割",
            Some(format!("Bearer mock.admin.{}", AccountId::from_u128(42))),
        ),
    ];
    for path in ["/any", "/coach-only"] {
        for (case, authorization) in &cases {
            let (status, www_authenticate, body) = get_with(path, authorization.as_deref()).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{path} {case}: {body}");
            assert_eq!(body["code"], "unauthorized", "{path} {case}");
            assert!(body["message"].is_string(), "{path} {case}");
            assert_eq!(www_authenticate.as_deref(), Some("Bearer"), "{path} {case}");
        }
    }
}

#[tokio::test]
async fn wrong_role_is_403_forbidden() {
    let (status, _, body) = get_with(
        "/coach-only",
        Some(&format!("Bearer {}", token(Role::Student))),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "forbidden");
}

#[tokio::test]
async fn valid_token_with_allowed_role_is_200() {
    for role in [Role::Coach, Role::Headquarters] {
        let (status, _, body) =
            get_with("/coach-only", Some(&format!("Bearer {}", token(role)))).await;
        assert_eq!(status, StatusCode::OK, "{role:?}: {body}");
        assert_eq!(body["role"], role.as_str());
        assert_eq!(body["account_id"], AccountId::from_u128(42).to_string());
    }

    // スキーム名は大文字小文字を問わない（RFC 6750）
    let (status, _, _) = get_with("/any", Some(&format!("bearer {}", token(Role::Student)))).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn token_from_registration_authenticates_the_student() {
    let (student_id, access_token) = register_student().await;
    let authorization = format!("Bearer {access_token}");

    let (status, _, body) = get_with("/any", Some(&authorization)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "account_id": student_id, "role": "student" }));

    let (status, _, body) = get_with("/coach-only", Some(&authorization)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}
