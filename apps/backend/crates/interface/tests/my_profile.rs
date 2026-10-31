//! `/me/profile` の生徒の分の統合テスト。student-app の `GET /me/profile`（`getMyStudentProfile`）と
//! `PATCH /me/profile`（`updateMyStudentProfile`）のレスポンスの形を OpenAPI のスキーマで検査し、生徒が自分の表示名を
//! 見て変えられること・空（空白だけ）・50 文字を超える・`display_name` がない表示名は 422 で何も変えないこと・
//! 生徒以外（コーチ・本部）は 403、トークンがなければ 401、トークンの生徒のアカウントがなければ 401 になることを確かめる。

use std::time::SystemTime;

use application::{
    identity::{
        AccessToken, AccountRepository, GetAccount, InvalidToken, TokenClaims, TokenIssuer,
        TokenVerifier, account_id_uuid,
    },
    in_memory::InMemoryAccountRepository,
    profile::UpdateStudentProfile,
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

const STUDENT: &str = "student:1";
const COACH: &str = "coach:9";
const HEADQUARTERS: &str = "headquarters:5";
/// アカウントのない生徒のトークン
const UNKNOWN_STUDENT: &str = "student:99";

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

fn account(token: &str, display_name: &str, role: Role, sports: &[Sport]) -> Account {
    let id = account_id(token);
    Account::new(
        id,
        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
        DisplayName::new(display_name).unwrap(),
        role,
        sports.iter().copied(),
        SystemTime::UNIX_EPOCH,
    )
    .unwrap()
}

async fn app() -> (Router, InMemoryAccountRepository) {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(STUDENT, "あおき", Role::Student, &[Sport::Tennis]),
        account(COACH, "たなか", Role::Coach, &[Sport::Soccer]),
        account(HEADQUARTERS, "うえの", Role::Headquarters, &[]),
    ] {
        accounts.insert(account).await.unwrap();
    }
    let router = interface::my_profile_router(
        GetAccount::new(accounts.clone()),
        UpdateStudentProfile::new(accounts.clone()),
        FakeTokens,
    );
    (router, accounts)
}

async fn send(
    router: &Router,
    method: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri("/me/profile")
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
    let body = serde_json::from_slice(&bytes).unwrap();
    support::Contract::load("student-app").assert_response(
        &method.to_lowercase(),
        "/me/profile",
        status,
        &body,
    );
    (status, body)
}

fn student_json(display_name: &str) -> Value {
    json!({
        "student_id": account_id_uuid::format(account_id(STUDENT)),
        "display_name": display_name,
        "sport": "tennis",
        "created_at": "1970-01-01T00:00:00Z"
    })
}

fn display_name_of(accounts: &InMemoryAccountRepository, token: &str) -> String {
    accounts
        .all()
        .into_iter()
        .find(|a| a.id() == account_id(token))
        .unwrap()
        .display_name()
        .as_str()
        .to_string()
}

#[tokio::test]
async fn a_student_gets_their_own_profile() {
    let (router, _) = app().await;
    let (status, body) = send(&router, "GET", Some(STUDENT), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, student_json("あおき"));
}

#[tokio::test]
async fn a_student_changes_their_display_name() {
    let (router, accounts) = app().await;
    let (status, body) = send(
        &router,
        "PATCH",
        Some(STUDENT),
        Some(json!({ "display_name": " じろう " })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, student_json(" じろう "), "書いたとおりに保存する");
    assert_eq!(display_name_of(&accounts, STUDENT), " じろう ");

    let (status, got) = send(&router, "GET", Some(STUDENT), None).await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_eq!(got, body);

    // 上限ちょうど（50 文字）まで書ける
    let full = "あ".repeat(DisplayName::MAX_CHARS);
    let (status, body) = send(
        &router,
        "PATCH",
        Some(STUDENT),
        Some(json!({ "display_name": full })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["display_name"], full);
}

#[tokio::test]
async fn empty_too_long_or_missing_display_name_is_unprocessable_and_nothing_changes() {
    let (router, accounts) = app().await;
    for request in [
        json!({ "display_name": "" }),
        json!({ "display_name": " \u{3000}" }),
        json!({ "display_name": "あ".repeat(DisplayName::MAX_CHARS + 1) }),
        json!({ "display_name": 1 }),
        json!({}),
    ] {
        let (status, body) = send(&router, "PATCH", Some(STUDENT), Some(request.clone())).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{request}: {body}"
        );
        assert_eq!(body["code"], "validation_failed", "{request}");
    }
    assert_eq!(display_name_of(&accounts, STUDENT), "あおき");
}

#[tokio::test]
async fn only_students_can_get_or_change_their_profile() {
    let (router, accounts) = app().await;
    let before = accounts.all();
    for token in [COACH, HEADQUARTERS] {
        let (status, body) = send(&router, "GET", Some(token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
        let (status, body) = send(
            &router,
            "PATCH",
            Some(token),
            Some(json!({ "display_name": "じろう" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
    assert_eq!(accounts.all(), before, "何も変えない");
}

#[tokio::test]
async fn missing_token_or_unknown_student_is_unauthorized() {
    let (router, accounts) = app().await;
    let before = accounts.all();
    for token in [None, Some(UNKNOWN_STUDENT)] {
        let (status, body) = send(&router, "GET", token, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
        let (status, body) = send(
            &router,
            "PATCH",
            token,
            Some(json!({ "display_name": "じろう" })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
    }
    assert_eq!(accounts.all(), before, "何も変えない");
}
