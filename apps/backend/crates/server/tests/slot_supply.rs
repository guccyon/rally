//! `GET /slot-supply` を組み立て済みのルーター（`server::router()`）で通す。トークンは infra のモック
//! （`MockTokenService`）で発行する。契約の形・数え方の検査は interface の `tests/slot_supply.rs`、
//! 合成シードでの判定は seed の `tests/slot_supply.rs`。

use application::identity::{TokenClaims, TokenIssuer};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role};
use infra::MockTokenService;
use serde_json::Value;
use tower::ServiceExt;

async fn get_slot_supply(role: Role) -> (StatusCode, Value) {
    let token = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role,
        })
        .into_string();
    let request = Request::get("/slot-supply")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = server::router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn headquarters_reads_the_supply_of_the_five_sports() {
    let (status, body) = get_slot_supply(Role::Headquarters).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sports: Vec<&str> = body["sports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["sport"].as_str().unwrap())
        .collect();
    assert_eq!(
        sports,
        ["tennis", "golf", "baseball", "soccer", "basketball"]
    );
}

#[tokio::test]
async fn coaches_and_students_may_not_read_the_supply() {
    for role in [Role::Coach, Role::Student] {
        let (status, body) = get_slot_supply(role).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{role:?}: {body}");
    }
}
