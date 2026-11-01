//! `GET /coaches/incomplete-profiles`（admin-web の `listIncompleteCoachProfiles`）の統合テスト。本部には、プロフィールに
//! 足りない項目のあるコーチを、空の項目の名前・最後に更新した時刻（今は記録していないので null）・登録した時刻とともに、
//! 登録の古い順で返すことを OpenAPI のスキーマで検査する。任意の `sport` で絞れ、`Sport` の値でなければ 422。
//! コーチ・生徒は 403、トークンがなければ 401。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::InMemoryAccountRepository,
    profile::{CoachProfileRepository, ListIncompleteCoachProfiles},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    profile::{CoachBio, DisplayName},
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

/// 2026-10-01T00:00:00Z から `hour` 時間後。
fn at(hour: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_812_800 + hour * 3600)
}

fn account(id: u128, role: Role, sports: &[Sport], registered_hour: u64) -> Account {
    Account::new(
        AccountId::from_u128(id),
        Email::new(format!("user{id}@example.com")).unwrap(),
        DisplayName::new(format!("なまえ{id}")).unwrap(),
        role,
        sports.iter().copied(),
        at(registered_hour),
    )
    .unwrap()
}

/// 本部（1）・生徒（2）と、コーチ 3 人。コーチ 3 は書き終え、4 は空白だけ、5 は書いていない（登録は 5 → 3 → 4 の順）。
async fn app() -> Router {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(1, Role::Headquarters, &[], 0),
        account(2, Role::Student, &[Sport::Tennis], 0),
        account(3, Role::Coach, &[Sport::Tennis], 2),
        account(4, Role::Coach, &[Sport::Golf], 3),
        account(5, Role::Coach, &[Sport::Soccer], 1),
    ] {
        accounts.insert(account).await.unwrap();
    }
    for (id, bio) in [(3, "テニス歴 20 年"), (4, "\u{3000} ")] {
        accounts
            .update_coach_bio(AccountId::from_u128(id), CoachBio::new(bio).unwrap())
            .await
            .unwrap();
    }
    interface::incomplete_coach_profiles_router(
        ListIncompleteCoachProfiles::new(accounts.clone(), accounts),
        FakeTokens,
    )
}

/// `GET /coaches/incomplete-profiles`。レスポンスを admin-web の契約で検査する。
async fn list(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    list_with_query(router, token, "").await
}

/// `GET /coaches/incomplete-profiles<query>`（`query` は空か `?` から始まるクエリ）。
async fn list_with_query(router: &Router, token: Option<&str>, query: &str) -> (StatusCode, Value) {
    let mut request = Request::get(format!("/coaches/incomplete-profiles{query}"));
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
        "/coaches/incomplete-profiles",
        status,
        &body,
    );
    (status, body)
}

#[tokio::test]
async fn headquarters_lists_coaches_with_missing_fields_oldest_registration_first() {
    let (status, body) = list(&app().await, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coaches": [
                {
                    "coach_id": "00000000-0000-0000-0000-000000000005",
                    "display_name": "なまえ5",
                    "missing_fields": ["bio"],
                    "updated_at": null,
                    "registered_at": "2026-10-01T01:00:00Z",
                },
                {
                    "coach_id": "00000000-0000-0000-0000-000000000004",
                    "display_name": "なまえ4",
                    "missing_fields": ["bio"],
                    "updated_at": null,
                    "registered_at": "2026-10-01T03:00:00Z",
                },
            ],
        }),
        "書き終えたコーチ・本部・生徒は載らない"
    );
}

#[tokio::test]
async fn no_incomplete_coaches_is_an_empty_list() {
    let router = interface::incomplete_coach_profiles_router(
        ListIncompleteCoachProfiles::new(
            InMemoryAccountRepository::default(),
            InMemoryAccountRepository::default(),
        ),
        FakeTokens,
    );
    let (status, body) = list(&router, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "coaches": [] }));
}

#[tokio::test]
async fn coaches_and_students_are_forbidden() {
    let router = app().await;
    for token in ["coach:3", "coach:5", "student:2"] {
        let (status, body) = list(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
}

#[tokio::test]
async fn missing_or_invalid_token_is_unauthorized() {
    let router = app().await;
    for token in [None, Some("nobody:1")] {
        let (status, body) = list(&router, token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}: {body}");
    }
}

#[tokio::test]
async fn sport_narrows_to_coaches_offering_it() {
    let router = app().await;
    let (status, body) = list_with_query(&router, Some(HEADQUARTERS), "?sport=golf").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["coaches"],
        json!([{
            "coach_id": "00000000-0000-0000-0000-000000000004",
            "display_name": "なまえ4",
            "missing_fields": ["bio"],
            "updated_at": null,
            "registered_at": "2026-10-01T03:00:00Z",
        }]),
        "ゴルフも絞れる"
    );

    let (status, body) = list_with_query(&router, Some(HEADQUARTERS), "?sport=tennis").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "coaches": [] }),
        "テニスのコーチは書き終えているので空"
    );
}

#[tokio::test]
async fn unknown_sport_is_unprocessable() {
    let router = app().await;
    for query in ["?sport=curling", "?sport=", "?sport=Tennis"] {
        let (status, body) = list_with_query(&router, Some(HEADQUARTERS), query).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
    }
}
