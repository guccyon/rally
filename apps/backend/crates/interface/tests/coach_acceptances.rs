//! `POST /coaches/{coach_id}/suspend`・`POST /coaches/{coach_id}/resume`（admin-web の `suspendCoach`・`resumeCoach`）と
//! `GET /coaches/{coach_id}/acceptance-changes`（`listCoachAcceptanceChanges`）の統合テスト。応答を admin-web の OpenAPI の
//! スキーマで検査する。どれも本部だけが呼べる（コーチ・生徒は 403）。停止の本文（任意の `reason`）は省いてよく、記録の一覧の
//! 1 件ごとに理由が載る（再開した記録は null）。

use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime},
};

use application::{
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{InMemoryAccountRepository, InMemoryCoachAcceptanceRepository, ManualClock},
    profile::ProfileCoachIntroductionLookup,
    scheduling::{
        CoachAcceptanceRepository, ListCoachAcceptanceChanges, ResumeCoach, SuspendCoach,
    },
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
const HEADQUARTERS_ID: &str = "00000000-0000-0000-0000-000000000001";
const OTHER_HEADQUARTERS: &str = "headquarters:4";
const OTHER_HEADQUARTERS_ID: &str = "00000000-0000-0000-0000-000000000004";
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

/// 時計の始まり（2026-11-01T00:00:00Z）。
fn start() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
}

/// 本部 1・4、コーチ 2・生徒 3 の保存先で組み立てたルーターと、コーチの受付の保存先と、時計。
async fn app() -> (Router, InMemoryCoachAcceptanceRepository, ManualClock) {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(1, Role::Headquarters, &[]),
        account(2, Role::Coach, &[Sport::Tennis]),
        account(3, Role::Student, &[Sport::Tennis]),
        account(4, Role::Headquarters, &[]),
    ] {
        accounts.insert(account).await.unwrap();
    }
    let acceptances = InMemoryCoachAcceptanceRepository::default();
    let coaches = ProfileCoachIntroductionLookup::new(accounts);
    let clock = ManualClock::new(start());
    let router = interface::coach_acceptance_router(
        SuspendCoach::new(acceptances.clone(), coaches.clone(), clock.clone()),
        ResumeCoach::new(acceptances.clone(), coaches.clone(), clock.clone()),
        ListCoachAcceptanceChanges::new(acceptances.clone(), coaches),
        FakeTokens,
    );
    (router, acceptances, clock)
}

/// `GET /coaches/{coach_id}/acceptance-changes`。レスポンスを admin-web の契約で検査する。
async fn get_changes(router: &Router, coach_id: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get(format!("/coaches/{coach_id}/acceptance-changes"));
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
        "get",
        "/coaches/{coach_id}/acceptance-changes",
        status,
        &body,
    );
    (status, body)
}

/// `POST /coaches/{coach_id}/<action>`（本文なし）。レスポンスを admin-web の契約で検査する。
async fn post(
    router: &Router,
    coach_id: &str,
    action: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    post_with_body(router, coach_id, action, token, None).await
}

/// `POST /coaches/{coach_id}/<action>`。`body` があれば JSON の本文として送る。レスポンスを admin-web の契約で検査する。
async fn post_with_body(
    router: &Router,
    coach_id: &str,
    action: &str,
    token: Option<&str>,
    body: Option<Body>,
) -> (StatusCode, Value) {
    let mut request = Request::post(format!("/coaches/{coach_id}/{action}"));
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = match body {
        Some(body) => {
            request = request.header(header::CONTENT_TYPE, "application/json");
            body
        }
        None => Body::empty(),
    };
    let response = router
        .clone()
        .oneshot(request.body(body).unwrap())
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
    let (router, acceptances, _) = app().await;

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

fn json_body(value: Value) -> Option<Body> {
    Some(Body::from(value.to_string()))
}

/// 本部 1 が `body` を本文にしてコーチ 2 の受付を止め、記録の一覧の最新の `reason` を返す。
async fn suspend_and_read_reason(body: Option<Body>) -> Value {
    let (router, _, _) = app().await;
    let (status, response) =
        post_with_body(&router, COACH_ID, "suspend", Some(HEADQUARTERS), body).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["acceptance_status"], "suspended");
    let (_, changes) = get_changes(&router, COACH_ID, Some(HEADQUARTERS)).await;
    changes["changes"][0]["reason"].clone()
}

#[tokio::test]
async fn headquarters_suspends_with_a_reason_kept_as_written() {
    assert_eq!(
        suspend_and_read_reason(json_body(json!({ "reason": " 無断欠席が続いた " }))).await,
        " 無断欠席が続いた ",
        "前後の空白も削らない"
    );
    assert_eq!(
        suspend_and_read_reason(json_body(json!({ "reason": "あ".repeat(200) }))).await,
        "あ".repeat(200),
        "200 文字までは通る"
    );
}

#[tokio::test]
async fn the_body_and_the_reason_may_be_omitted_as_an_empty_reason() {
    for body in [
        None,
        Some(Body::from(" ")),
        json_body(json!({})),
        json_body(json!({ "reason": null })),
        json_body(json!({ "reason": "" })),
    ] {
        assert_eq!(suspend_and_read_reason(body).await, "");
    }
}

#[tokio::test]
async fn a_too_long_reason_or_a_broken_body_is_422_and_changes_nothing() {
    let (router, acceptances, _) = app().await;
    for body in [
        json_body(json!({ "reason": "あ".repeat(201) })),
        Some(Body::from("{")),
        json_body(json!({ "reason": 1 })),
    ] {
        let (status, body) =
            post_with_body(&router, COACH_ID, "suspend", Some(HEADQUARTERS), body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["code"], "validation_failed");
    }
    assert!(acceptances.suspended_coaches().await.unwrap().is_empty());
    let (_, changes) = get_changes(&router, COACH_ID, Some(HEADQUARTERS)).await;
    assert_eq!(changes["changes"], json!([]), "断った操作は記録しない");
}

#[tokio::test]
async fn suspending_twice_or_resuming_an_accepting_coach_is_409() {
    let (router, acceptances, _) = app().await;

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
    let (router, acceptances, _) = app().await;
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
    let (router, acceptances, _) = app().await;
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

#[tokio::test]
async fn headquarters_lists_who_suspended_and_resumed_a_coach_why_and_when_newest_first() {
    let (router, _, clock) = app().await;

    // 一度も止めていないコーチは空の一覧
    let (status, body) = get_changes(&router, COACH_ID, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "coach_id": COACH_ID, "changes": [] }));

    post_with_body(
        &router,
        COACH_ID,
        "suspend",
        Some(HEADQUARTERS),
        json_body(json!({ "reason": "休業の申し出" })),
    )
    .await;
    clock.set(start() + Duration::from_secs(90 * 60));
    post(&router, COACH_ID, "resume", Some(OTHER_HEADQUARTERS)).await;
    // 断られた操作は残らない
    post(&router, COACH_ID, "resume", Some(HEADQUARTERS)).await;

    let (status, body) = get_changes(&router, COACH_ID, Some(OTHER_HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coach_id": COACH_ID,
            "changes": [
                {
                    "acceptance_status": "accepting",
                    "reason": null,
                    "changed_by": OTHER_HEADQUARTERS_ID,
                    "changed_at": "2026-11-01T01:30:00Z",
                },
                {
                    "acceptance_status": "suspended",
                    "reason": "休業の申し出",
                    "changed_by": HEADQUARTERS_ID,
                    "changed_at": "2026-11-01T00:00:00Z",
                },
            ],
        })
    );
}

#[tokio::test]
async fn listing_changes_of_a_non_coach_missing_or_malformed_id_is_404() {
    let (router, _, _) = app().await;
    for coach_id in [
        STUDENT_ID,
        "00000000-0000-0000-0000-000000000099",
        "not-a-uuid",
    ] {
        let (status, body) = get_changes(&router, coach_id, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{coach_id}: {body}");
        assert_eq!(body["code"], "not_found");
    }
}

#[tokio::test]
async fn only_headquarters_may_list_changes_and_a_token_is_required() {
    let (router, _, _) = app().await;
    for token in ["coach:2", "student:3"] {
        let (status, body) = get_changes(&router, COACH_ID, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
    let (status, body) = get_changes(&router, COACH_ID, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}
