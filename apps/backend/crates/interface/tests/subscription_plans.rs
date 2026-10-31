//! `/subscription-plans` の統合テスト。student-app の `GET /subscription-plans`（`listSubscriptionPlans`）のレスポンスの
//! 形を OpenAPI のスキーマで検査し、生徒には契約プランを standard・plus の順に各プランのビデオ通話の暦月の上限とともに
//! 返すこと・生徒以外（コーチ・本部）は 403、トークンがなければ 401 になることを確かめる。

use application::{
    entitlement::ListSubscriptionPlans,
    identity::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// `<役割>:<ID の数値>` の形のトークンを通す偽物（interface は infra のモックに依存できない）。
struct FakeTokens;

impl TokenIssuer for FakeTokens {
    fn issue(&self, _claims: TokenClaims) -> AccessToken {
        unreachable!("このテストでは発行しない")
    }
}

impl TokenVerifier for FakeTokens {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken> {
        let (role, id) = token.split_once(':').ok_or(InvalidToken)?;
        let role = match role {
            "student" => Role::Student,
            "coach" => Role::Coach,
            "headquarters" => Role::Headquarters,
            _ => return Err(InvalidToken),
        };
        let id = id.parse().map_err(|_| InvalidToken)?;
        Ok(TokenClaims {
            account_id: AccountId::from_u128(id),
            role,
        })
    }
}

const STUDENT: &str = "student:1";
const COACH: &str = "coach:9";
const HEADQUARTERS: &str = "headquarters:5";

fn app() -> Router {
    interface::subscription_plans_router(ListSubscriptionPlans::new(), FakeTokens)
}

async fn list_subscription_plans(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/subscription-plans");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    support::Contract::load("student-app").assert_response(
        "get",
        "/subscription-plans",
        status,
        &body,
    );
    (status, body)
}

#[tokio::test]
async fn a_student_gets_the_plans_with_their_monthly_live_lesson_limits() {
    let (status, body) = list_subscription_plans(&app(), Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "plans": [
                { "plan": "standard", "live_lessons_per_month": 2 },
                { "plan": "plus", "live_lessons_per_month": 4 }
            ]
        }),
        "standard・plus の順に不変条件 1 の上限で"
    );
}

#[tokio::test]
async fn only_students_can_list_the_plans() {
    let router = app();
    for token in [COACH, HEADQUARTERS] {
        let (status, body) = list_subscription_plans(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
    }
    let (status, body) = list_subscription_plans(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}
