//! `/coaches/{coach_id}` の統合テスト。student-app の `GET /coaches/{coach_id}`（`getCoachProfile`）のレスポンスの形を
//! OpenAPI のスキーマで検査し、コーチの表示名・自己紹介文（書いていなければ null）・提供スポーツ（ゴルフを除く）と、
//! トークンの生徒のお気に入りかを返すこと・
//! コーチでない ID・ない ID・UUID でない ID・ゴルフだけのコーチは 404・生徒以外（コーチ・本部）は 403、トークンが
//! なければ 401 になることを確かめる。

use std::time::SystemTime;

use application::{
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{InMemoryAccountRepository, InMemoryFavoriteCoachRepository},
    matching::MatchingCoachFavoriteLookup,
    profile::GetCoachProfile,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    matching::FavoriteCoach,
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

/// お気に入りは自己紹介文を書いたコーチ（9）だけ
const STUDENT: &str = "student:1";
/// お気に入りのない生徒
const OTHER_STUDENT: &str = "student:2";
/// 自己紹介文を書いたコーチ（テニス・ゴルフ・サッカー）
const COACH: &str = "coach:9";
/// 自己紹介文を書いていないコーチ（水泳）
const QUIET_COACH: &str = "coach:8";
/// ゴルフだけのコーチ（golf-app の範囲）
const GOLF_COACH: &str = "coach:7";
const HEADQUARTERS: &str = "headquarters:5";

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

async fn app() -> Router {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(STUDENT, "あおき", Role::Student, &[Sport::Tennis]),
        account(OTHER_STUDENT, "えもと", Role::Student, &[Sport::Soccer]),
        account(
            COACH,
            "たなか",
            Role::Coach,
            &[Sport::Soccer, Sport::Golf, Sport::Tennis],
        ),
        account(QUIET_COACH, "いとう", Role::Coach, &[Sport::Swimming]),
        account(GOLF_COACH, "うえだ", Role::Coach, &[Sport::Golf]),
        account(HEADQUARTERS, "うえの", Role::Headquarters, &[]),
    ] {
        accounts.insert(account).await.unwrap();
    }
    accounts
        .update_coach_bio(
            account_id(COACH),
            CoachBio::new("テニス歴 20 年。初心者歓迎").unwrap(),
        )
        .await
        .unwrap();
    let favorites = InMemoryFavoriteCoachRepository::with_favorites([FavoriteCoach::restore(
        account_id(STUDENT),
        account_id(COACH),
        SystemTime::UNIX_EPOCH,
    )]);
    interface::coach_profile_router(
        GetCoachProfile::new(accounts, MatchingCoachFavoriteLookup::new(favorites)),
        FakeTokens,
    )
}

async fn get_coach_profile(
    router: &Router,
    coach_id: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::get(format!("/coaches/{coach_id}"));
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
        "/coaches/{coach_id}",
        status,
        &body,
    );
    (status, body)
}

fn coach_id(token: &str) -> String {
    account_id_uuid::format(account_id(token))
}

#[tokio::test]
async fn a_student_gets_the_display_name_bio_and_sports_of_a_coach() {
    let router = app().await;
    let (status, body) = get_coach_profile(&router, &coach_id(COACH), Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coach_id": coach_id(COACH),
            "display_name": "たなか",
            "bio": "テニス歴 20 年。初心者歓迎",
            "sports": ["tennis", "soccer"],
            "is_favorite": true
        }),
        "スポーツは Sport の順で、ゴルフは載らない"
    );

    let (status, body) = get_coach_profile(&router, &coach_id(COACH), Some(OTHER_STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["is_favorite"], false, "お気に入りは生徒ごと");
}

#[tokio::test]
async fn a_coach_without_a_bio_has_null_bio() {
    let router = app().await;
    let (status, body) = get_coach_profile(&router, &coach_id(QUIET_COACH), Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coach_id": coach_id(QUIET_COACH),
            "display_name": "いとう",
            "bio": null,
            "sports": ["swimming"],
            "is_favorite": false
        })
    );
}

#[tokio::test]
async fn non_coach_unknown_golf_only_or_malformed_ids_are_not_found() {
    let router = app().await;
    for id in [
        coach_id(STUDENT),
        coach_id(HEADQUARTERS),
        coach_id(GOLF_COACH),
        account_id_uuid::format(AccountId::from_u128(99)),
        "not-a-uuid".to_string(),
    ] {
        let (status, body) = get_coach_profile(&router, &id, Some(STUDENT)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {body}");
    }
}

#[tokio::test]
async fn only_students_can_get_a_coach_profile() {
    let router = app().await;
    for token in [COACH, QUIET_COACH, HEADQUARTERS] {
        let (status, body) = get_coach_profile(&router, &coach_id(COACH), Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
    }
    let (status, body) = get_coach_profile(&router, &coach_id(COACH), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}
