//! `GET /subscription-plans` を組み立て済みのルーター（`server::router()`）で通す。生徒は受講権がなくても（契約の前でも）
//! 契約プランの一覧を受け取り、コーチは 403。契約の形の検査は interface の `tests/subscription_plans.rs`。

use application::identity::{TokenClaims, TokenIssuer};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role};
use infra::MockTokenService;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn list_subscription_plans(router: &Router, role: Role) -> (StatusCode, Value) {
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role,
        })
        .into_string();
    let response = router
        .clone()
        .oneshot(
            Request::get("/subscription-plans")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn student_without_an_entitlement_gets_the_plans() {
    let (status, body) = list_subscription_plans(&server::router(), Role::Student).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "plans": [
                { "plan": "standard", "live_lessons_per_month": 2 },
                { "plan": "plus", "live_lessons_per_month": 4 }
            ]
        })
    );
}

#[tokio::test]
async fn coach_is_forbidden() {
    let (status, body) = list_subscription_plans(&server::router(), Role::Coach).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}
