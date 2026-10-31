//! `/favorite-coaches` の統合テスト。student-app の `GET /favorite-coaches`（`listFavoriteCoaches`）と
//! `PUT`・`DELETE /favorite-coaches/{coach_id}`（`addFavoriteCoach`・`removeFavoriteCoach`）のレスポンスの形を OpenAPI の
//! スキーマで検査し、入れた・外したあとの一覧を返すこと・同じコーチを 2 回入れても 1 件で最初の時刻のまま・新しい順・
//! ゴルフを載せないこと・コーチでない ID・ない ID・ゴルフだけのコーチ・UUID でない ID は 404・アカウントのない生徒は 401・
//! 生徒以外（コーチ・本部）は 403、トークンがなければ 401 になることを確かめる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        InMemoryAccountRepository, InMemoryFavoriteCoachRepository, InMemoryLessonReviewRepository,
        ManualClock,
    },
    matching::{AddFavoriteCoach, ListFavoriteCoaches, RemoveFavoriteCoach},
    review::ReviewCoachReviewSummaryLookup,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    lesson::LessonCompletionId,
    profile::DisplayName,
    review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
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
const OTHER_STUDENT: &str = "student:2";
/// テニス・ゴルフのコーチ（評価 2 件）
const COACH: &str = "coach:9";
/// 水泳のコーチ（評価なし）
const SWIM_COACH: &str = "coach:8";
/// ゴルフだけのコーチ（golf-app の範囲）
const GOLF_COACH: &str = "coach:7";
const HEADQUARTERS: &str = "headquarters:5";

fn account_id(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

fn coach_id(token: &str) -> String {
    account_id_uuid::format(account_id(token))
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

fn review(id: u128, coach: &str, rating: i64) -> LessonReview {
    LessonReview::restore(
        LessonReviewId::from_u128(id),
        LessonCompletionId::from_u128(id),
        account_id(OTHER_STUDENT),
        account_id(coach),
        Rating::new(rating).unwrap(),
        LessonReviewComment::new("よい").unwrap(),
        SystemTime::UNIX_EPOCH,
    )
}

/// 2026-11-01T00:00:00Z から `minutes` 分後。
fn at(minutes: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200 + minutes * 60)
}

async fn app(clock: ManualClock) -> Router {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(STUDENT, "あおき", Role::Student, &[Sport::Tennis]),
        account(OTHER_STUDENT, "えもと", Role::Student, &[Sport::Soccer]),
        account(COACH, "たなか", Role::Coach, &[Sport::Golf, Sport::Tennis]),
        account(SWIM_COACH, "いとう", Role::Coach, &[Sport::Swimming]),
        account(GOLF_COACH, "うえだ", Role::Coach, &[Sport::Golf]),
        account(HEADQUARTERS, "うえの", Role::Headquarters, &[]),
    ] {
        accounts.insert(account).await.unwrap();
    }
    let favorites = InMemoryFavoriteCoachRepository::default();
    let reviews =
        ReviewCoachReviewSummaryLookup::new(InMemoryLessonReviewRepository::with_reviews([
            review(1, COACH, 5),
            review(2, COACH, 4),
        ]));
    interface::favorite_coaches_router(
        AddFavoriteCoach::new(favorites.clone(), accounts.clone(), clock),
        RemoveFavoriteCoach::new(favorites.clone()),
        ListFavoriteCoaches::new(favorites, accounts, reviews),
        FakeTokens,
    )
}

/// `method` を `path` に送り、レスポンスを student-app の契約（`contract_path` の操作）で検査する。
async fn send(
    router: &Router,
    method: Method,
    coach: Option<&str>,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let (uri, contract_path) = match coach {
        Some(coach) => (
            format!("/favorite-coaches/{coach}"),
            "/favorite-coaches/{coach_id}",
        ),
        None => ("/favorite-coaches".to_string(), "/favorite-coaches"),
    };
    let mut request = Request::builder().method(method.clone()).uri(uri);
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
        &method.as_str().to_lowercase(),
        contract_path,
        status,
        &body,
    );
    (status, body)
}

fn coach_ids(body: &Value) -> Vec<String> {
    body["favorite_coaches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["coach_id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn a_student_adds_coaches_once_and_lists_them_newest_first() {
    let clock = ManualClock::new(at(0));
    let router = app(clock.clone()).await;

    let (status, body) = send(&router, Method::GET, None, Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "favorite_coaches": [] }));

    let (status, body) = send(&router, Method::PUT, Some(&coach_id(COACH)), Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "favorite_coaches": [{
                "coach_id": coach_id(COACH),
                "display_name": "たなか",
                "sports": ["tennis"],
                "review_count": 2,
                "average_rating": 4.5,
                "favorited_at": "2026-11-01T00:00:00Z",
            }],
        }),
        "入れたあとの一覧を返す。ゴルフは載らない"
    );

    clock.set(at(10));
    send(
        &router,
        Method::PUT,
        Some(&coach_id(SWIM_COACH)),
        Some(STUDENT),
    )
    .await;
    clock.set(at(20));
    // 同じコーチをもう一度入れても 1 件で、最初の時刻のまま
    let (status, body) = send(&router, Method::PUT, Some(&coach_id(COACH)), Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(coach_ids(&body), [coach_id(SWIM_COACH), coach_id(COACH)]);
    assert_eq!(
        body["favorite_coaches"][0],
        json!({
            "coach_id": coach_id(SWIM_COACH),
            "display_name": "いとう",
            "sports": ["swimming"],
            "review_count": 0,
            "average_rating": null,
            "favorited_at": "2026-11-01T00:10:00Z",
        })
    );
    assert_eq!(
        body["favorite_coaches"][1]["favorited_at"],
        "2026-11-01T00:00:00Z"
    );

    let (status, body) = send(&router, Method::GET, None, Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(coach_ids(&body), [coach_id(SWIM_COACH), coach_id(COACH)]);

    let (status, body) = send(&router, Method::GET, None, Some(OTHER_STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "favorite_coaches": [] }),
        "ほかの生徒のお気に入りは見えない"
    );
}

#[tokio::test]
async fn a_student_removes_a_coach_and_removing_again_is_ok() {
    let router = app(ManualClock::new(at(0))).await;
    send(&router, Method::PUT, Some(&coach_id(COACH)), Some(STUDENT)).await;
    send(
        &router,
        Method::PUT,
        Some(&coach_id(SWIM_COACH)),
        Some(STUDENT),
    )
    .await;
    send(
        &router,
        Method::PUT,
        Some(&coach_id(COACH)),
        Some(OTHER_STUDENT),
    )
    .await;

    let (status, body) = send(
        &router,
        Method::DELETE,
        Some(&coach_id(COACH)),
        Some(STUDENT),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        coach_ids(&body),
        [coach_id(SWIM_COACH)],
        "外したあとの一覧を返す"
    );

    for id in [
        coach_id(COACH),
        account_id_uuid::format(AccountId::from_u128(99)),
    ] {
        let (status, body) = send(&router, Method::DELETE, Some(&id), Some(STUDENT)).await;
        assert_eq!(status, StatusCode::OK, "{id}: {body}");
        assert_eq!(coach_ids(&body), [coach_id(SWIM_COACH)]);
    }

    let (_, body) = send(&router, Method::GET, None, Some(OTHER_STUDENT)).await;
    assert_eq!(
        coach_ids(&body),
        [coach_id(COACH)],
        "ほかの生徒のお気に入りは変えない"
    );
}

#[tokio::test]
async fn non_coach_unknown_golf_only_or_malformed_ids_are_not_found() {
    let router = app(ManualClock::new(at(0))).await;
    for id in [
        coach_id(OTHER_STUDENT),
        coach_id(HEADQUARTERS),
        coach_id(GOLF_COACH),
        account_id_uuid::format(AccountId::from_u128(99)),
        "not-a-uuid".to_string(),
    ] {
        let (status, body) = send(&router, Method::PUT, Some(&id), Some(STUDENT)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {body}");
        assert_eq!(body["code"], "not_found");
    }
    let (status, body) = send(&router, Method::DELETE, Some("not-a-uuid"), Some(STUDENT)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    let (_, body) = send(&router, Method::GET, None, Some(STUDENT)).await;
    assert_eq!(body, json!({ "favorite_coaches": [] }), "何も入らない");
}

#[tokio::test]
async fn a_student_without_an_account_is_401() {
    let router = app(ManualClock::new(at(0))).await;
    let (status, body) = send(
        &router,
        Method::PUT,
        Some(&coach_id(COACH)),
        Some("student:99"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn only_students_can_use_favorites() {
    let router = app(ManualClock::new(at(0))).await;
    for token in [Some(COACH), Some(HEADQUARTERS), None] {
        let expected = if token.is_some() {
            StatusCode::FORBIDDEN
        } else {
            StatusCode::UNAUTHORIZED
        };
        let (status, body) = send(&router, Method::GET, None, token).await;
        assert_eq!(status, expected, "{token:?}: {body}");
        for method in [Method::PUT, Method::DELETE] {
            let (status, body) =
                send(&router, method.clone(), Some(&coach_id(SWIM_COACH)), token).await;
            assert_eq!(status, expected, "{method} {token:?}: {body}");
        }
    }
}
