//! `POST /coaches/{coach_id}/suspend`・`POST /coaches/{coach_id}/resume`（admin-web の `suspendCoach`・`resumeCoach`）の
//! 統合テスト。応答を admin-web の OpenAPI のスキーマで検査する。本部だけが呼べる（コーチ・生徒は 403）。

use std::{collections::BTreeSet, time::SystemTime};

use application::{
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{InMemoryAccountRepository, InMemoryCoachAcceptanceRepository},
    profile::ProfileCoachIntroductionLookup,
    scheduling::{CoachAcceptanceRepository, ResumeCoach, SuspendCoach},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    profile::DisplayName,
};
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

const HEADQUARTERS: &str = "headquarters:1";
const COACH_ID: &str = "00000000-0000-0000-0000-000000000002";
const STUDENT_ID: &str = "00000000-0000-0000-0000-000000000003";

fn account(id: u128, role: Role, sports: &[Sport]) -> Account {
    Account::new(
        AccountId::from_u128(id),
        Email::new(format!("user{id}@example.com")).unwrap(),
        DisplayName::new("テスト").unwrap(),
        role,
        sports.iter().copied(),
        SystemTime::UNIX_EPOCH,
    )
    .unwrap()
}

/// 本部 1・コーチ 2・生徒 3 の保存先で組み立てたルーターと、コーチの受付の保存先。
async fn app() -> (Router, InMemoryCoachAcceptanceRepository) {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(1, Role::Headquarters, &[]),
        account(2, Role::Coach, &[Sport::Tennis]),
        account(3, Role::Student, &[Sport::Tennis]),
    ] {
        accounts.insert(account).await.unwrap();
    }
    let acceptances = InMemoryCoachAcceptanceRepository::default();
    let coaches = ProfileCoachIntroductionLookup::new(accounts);
    let router = interface::coach_acceptance_router(
        SuspendCoach::new(acceptances.clone(), coaches.clone()),
        ResumeCoach::new(acceptances.clone(), coaches),
        FakeTokens,
    );
    (router, acceptances)
}

/// `POST /coaches/{coach_id}/<action>`。レスポンスを admin-web の契約で検査する。
async fn post(
    router: &Router,
    coach_id: &str,
    action: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::post(format!("/coaches/{coach_id}/{action}"));
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
    support::Contract::load("admin-web").assert_response(
        "post",
        &format!("/coaches/{{coach_id}}/{action}"),
        status,
        &body,
    );
    (status, body)
}

#[tokio::test]
async fn headquarters_suspends_and_resumes_a_coach() {
    let (router, acceptances) = app().await;

    let (status, body) = post(&router, COACH_ID, "suspend", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "coach_id": COACH_ID, "acceptance_status": "suspended" })
    );
    assert_eq!(
        acceptances.suspended_coaches().await.unwrap(),
        BTreeSet::from([AccountId::from_u128(2)])
    );

    let (status, body) = post(&router, COACH_ID, "resume", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "coach_id": COACH_ID, "acceptance_status": "accepting" })
    );
    assert!(acceptances.suspended_coaches().await.unwrap().is_empty());
}

#[tokio::test]
async fn suspending_twice_or_resuming_an_accepting_coach_is_409() {
    let (router, acceptances) = app().await;

    let (status, body) = post(&router, COACH_ID, "resume", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "coach_not_suspended");

    post(&router, COACH_ID, "suspend", Some(HEADQUARTERS)).await;
    let (status, body) = post(&router, COACH_ID, "suspend", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "coach_already_suspended");
    assert_eq!(
        acceptances.suspended_coaches().await.unwrap(),
        BTreeSet::from([AccountId::from_u128(2)]),
        "失敗しても変えない"
    );
}

#[tokio::test]
async fn a_non_coach_missing_or_malformed_id_is_404() {
    let (router, acceptances) = app().await;
    for action in ["suspend", "resume"] {
        for coach_id in [
            STUDENT_ID,
            "00000000-0000-0000-0000-000000000099",
            "not-a-uuid",
        ] {
            let (status, body) = post(&router, coach_id, action, Some(HEADQUARTERS)).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{action} {coach_id}: {body}");
            assert_eq!(body["code"], "not_found");
        }
    }
    assert!(acceptances.suspended_coaches().await.unwrap().is_empty());
}

#[tokio::test]
async fn only_headquarters_may_call_and_a_token_is_required() {
    let (router, acceptances) = app().await;
    for action in ["suspend", "resume"] {
        for token in ["coach:2", "student:3"] {
            let (status, body) = post(&router, COACH_ID, action, Some(token)).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{action} {token}: {body}");
            assert_eq!(body["code"], "forbidden");
        }
        let (status, body) = post(&router, COACH_ID, action, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{action}: {body}");
        assert_eq!(body["code"], "unauthorized");
    }
    assert!(acceptances.suspended_coaches().await.unwrap().is_empty());
}
