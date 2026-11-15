//! `AuthenticatedAccount`（抽出子）と `require_role`（層）のテスト。トークンはテスト用の偽の
//! 検証器で検証する（interface は infra のモックに依存できない）。infra の実装を通す統合テストは
//! server の `tests/auth.rs`。

use std::sync::Arc;

use application::identity::{InvalidToken, TokenClaims, TokenVerifier, account_id_uuid};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    routing::get,
};
use domain::identity::{AccountId, Role};
use interface::{AuthenticatedAccount, auth::TokenVerification, require_role};
use serde_json::{Value, json};
use tower::ServiceExt;

const ACCOUNT_ID: AccountId = AccountId::from_u128(42);

/// 決まったトークンだけを通す偽の検証器（`student` / `coach` / `headquarters` → 各役割の ID 42）。
struct FakeVerifier;

impl TokenVerifier for FakeVerifier {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken> {
        let role = match token {
            "student" => Role::Student,
            "coach" => Role::Coach,
            "headquarters" => Role::Headquarters,
            _ => return Err(InvalidToken),
        };
        Ok(TokenClaims {
            account_id: ACCOUNT_ID,
            role,
        })
    }
}

async fn whoami(account: AuthenticatedAccount) -> Json<Value> {
    Json(json!({
        "account_id": account_id_uuid::format(account.account_id()),
        "role": format!("{:?}", account.role()),
    }))
}

/// `/any` は認証だけ、`/coach-only` はコーチと本部だけ。
fn protected() -> Router {
    Router::new()
        .route("/coach-only", get(whoami))
        .route_layer(require_role(&[Role::Coach, Role::Headquarters]))
        .route("/any", get(whoami))
        .layer(TokenVerification::layer(Arc::new(FakeVerifier)))
}

async fn get_with(path: &str, authorization: Option<&str>) -> (StatusCode, Option<String>, Value) {
    let mut request = Request::get(path);
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, value);
    }
    let response = protected()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
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

#[tokio::test]
async fn missing_or_invalid_token_is_401_unauthorized() {
    let cases = [
        ("Authorization なし", None),
        ("Bearer でない", Some("Basic coach")),
        ("トークンが空", Some("Bearer ")),
        ("検証に通らない", Some("Bearer unknown")),
    ];
    for path in ["/any", "/coach-only"] {
        for (case, authorization) in cases {
            let (status, www_authenticate, body) = get_with(path, authorization).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{path} {case}: {body}");
            assert_eq!(body["code"], "unauthorized", "{path} {case}");
            assert!(body["message"].is_string(), "{path} {case}");
            assert_eq!(www_authenticate.as_deref(), Some("Bearer"), "{path} {case}");
        }
    }
}

#[tokio::test]
async fn wrong_role_is_403_forbidden() {
    let (status, www_authenticate, body) = get_with("/coach-only", Some("Bearer student")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "forbidden");
    assert!(body["message"].is_string());
    assert_eq!(www_authenticate, None);
}

#[tokio::test]
async fn valid_token_with_allowed_role_is_200() {
    for (token, role) in [("coach", Role::Coach), ("headquarters", Role::Headquarters)] {
        let (status, _, body) = get_with("/coach-only", Some(&format!("Bearer {token}"))).await;
        assert_eq!(status, StatusCode::OK, "{role:?}: {body}");
        assert_eq!(
            body,
            json!({
                "account_id": account_id_uuid::format(ACCOUNT_ID),
                "role": format!("{role:?}"),
            })
        );
    }

    // 認証だけのルートは役割を問わない。スキーム名は大文字小文字を問わない（RFC 6750）
    let (status, _, body) = get_with("/any", Some("bearer student")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["role"], "Student");
}
