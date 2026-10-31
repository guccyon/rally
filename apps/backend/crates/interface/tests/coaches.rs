//! `GET /coaches` の統合テスト。本部には admin-web の `listCoaches`、生徒には student-app の
//! `listCoachesForStudent` の形で返すことを、それぞれの OpenAPI のスキーマで検査する。生徒にはその生徒のお気に入りかを
//! 添える。コーチは 403。

use std::time::SystemTime;

use application::{
    catalog::CatalogPublishedLessonMenuCountLookup,
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{
        InMemoryAccountRepository, InMemoryCoachAcceptanceRepository,
        InMemoryFavoriteCoachRepository, InMemoryLessonMenuRepository,
        InMemoryLessonReviewRepository,
    },
    matching::ListCoachesForStudent,
    profile::{ListCoaches, ProfileCoachCandidateLookup, ProfileStudentSportLookup},
    review::ReviewCoachReviewSummaryLookup,
    scheduling::{CoachAcceptanceRepository, SchedulingCoachAcceptanceLookup},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::{
        LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId,
        LessonMenuName, Sport,
    },
    identity::{Account, AccountId, Email, Role},
    lesson::LessonCompletionId,
    matching::FavoriteCoach,
    profile::DisplayName,
    review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 受付を止める・再開する本部のアカウント。
const HEADQUARTERS_ID: AccountId = AccountId::from_u128(900);

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
const STUDENT: &str = "student:3";
const GOLF_STUDENT: &str = "student:6";

fn account(id: u128, display_name: &str, role: Role, sports: &[Sport]) -> Account {
    Account::new(
        AccountId::from_u128(id),
        Email::new(format!("user{id}@example.com")).unwrap(),
        DisplayName::new(display_name).unwrap(),
        role,
        sports.iter().copied(),
        SystemTime::UNIX_EPOCH,
    )
    .unwrap()
}

/// コーチ 3 人（うち 2 人は同じ表示名）と、テニスの生徒・ゴルフの生徒（seed などでしか作れない）・本部が 1 人ずつ。
async fn accounts() -> InMemoryAccountRepository {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(1, "本部", Role::Headquarters, &[]),
        account(2, "たなか", Role::Coach, &[Sport::Tennis]),
        account(3, "あおき", Role::Student, &[Sport::Tennis]),
        account(5, "いとう", Role::Coach, &[Sport::Soccer, Sport::Golf]),
        account(4, "いとう", Role::Coach, &[Sport::Baseball, Sport::Tennis]),
        account(6, "ごるふ", Role::Student, &[Sport::Golf]),
    ] {
        accounts.insert(account).await.unwrap();
    }
    accounts
}

fn review(id: u128, coach_id: u128, rating: i64) -> LessonReview {
    LessonReview::restore(
        LessonReviewId::from_u128(id),
        LessonCompletionId::from_u128(id),
        AccountId::from_u128(3),
        AccountId::from_u128(coach_id),
        Rating::new(rating).unwrap(),
        LessonReviewComment::new("よい").unwrap(),
        SystemTime::UNIX_EPOCH,
    )
}

fn menu(id: u128, coach_id: u128, sport: Sport, is_published: bool) -> LessonMenu {
    LessonMenu::restore(
        LessonMenuId::from_u128(id),
        AccountId::from_u128(coach_id),
        LessonMenuContent {
            sport,
            format: LessonFormat::Async,
            name: LessonMenuName::new("添削").unwrap(),
            description: LessonMenuDescription::default(),
        },
        is_published,
        SystemTime::UNIX_EPOCH,
    )
}

/// テニスの生徒（3）のお気に入りはコーチ 4 だけ（ほかの生徒のお気に入りは数えない）。
fn app(accounts: InMemoryAccountRepository) -> Router {
    app_with(accounts, InMemoryCoachAcceptanceRepository::default())
}

/// [`app`] と同じで、コーチの受付の状態は `acceptances`。
fn app_with(
    accounts: InMemoryAccountRepository,
    acceptances: InMemoryCoachAcceptanceRepository,
) -> Router {
    let reviews = InMemoryLessonReviewRepository::with_reviews([
        review(1, 2, 4),
        review(2, 2, 3),
        review(3, 4, 5),
        review(4, 5, 2),
    ]);
    let menus = InMemoryLessonMenuRepository::with_menus([
        menu(1, 2, Sport::Tennis, true),
        menu(2, 2, Sport::Tennis, false),
        menu(3, 5, Sport::Soccer, true),
        menu(4, 5, Sport::Golf, true),
    ]);
    let acceptances = SchedulingCoachAcceptanceLookup::new(acceptances);
    interface::coaches_router(
        ListCoaches::new(accounts.clone(), acceptances.clone()),
        ListCoachesForStudent::new(
            ProfileStudentSportLookup::new(accounts.clone()),
            ProfileCoachCandidateLookup::new(accounts),
            ReviewCoachReviewSummaryLookup::new(reviews),
            CatalogPublishedLessonMenuCountLookup::new(menus),
            InMemoryFavoriteCoachRepository::with_favorites([
                FavoriteCoach::restore(
                    AccountId::from_u128(3),
                    AccountId::from_u128(4),
                    SystemTime::UNIX_EPOCH,
                ),
                FavoriteCoach::restore(
                    AccountId::from_u128(6),
                    AccountId::from_u128(2),
                    SystemTime::UNIX_EPOCH,
                ),
            ]),
            acceptances,
        ),
        FakeTokens,
    )
}

/// `GET /coaches<query>`。レスポンスを `app`（admin-web / student-app）の契約で検査する。
async fn list_coaches(
    router: &Router,
    app: &str,
    query: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::get(format!("/coaches{query}"));
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
    support::Contract::load(app).assert_response("get", "/coaches", status, &body);
    (status, body)
}

#[tokio::test]
async fn headquarters_lists_only_coaches_in_display_name_order() {
    let router = app(accounts().await);
    let (status, body) = list_coaches(&router, "admin-web", "", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coaches": [
                // 同じ表示名は ID の順
                {
                    "coach_id": "00000000-0000-0000-0000-000000000004",
                    "display_name": "いとう",
                    "acceptance_status": "accepting",
                },
                {
                    "coach_id": "00000000-0000-0000-0000-000000000005",
                    "display_name": "いとう",
                    "acceptance_status": "accepting",
                },
                {
                    "coach_id": "00000000-0000-0000-0000-000000000002",
                    "display_name": "たなか",
                    "acceptance_status": "accepting",
                },
            ],
        })
    );
}

#[tokio::test]
async fn suspended_coaches_are_listed_to_headquarters_with_their_status_but_not_to_students() {
    let acceptances = InMemoryCoachAcceptanceRepository::default();
    acceptances
        .update(AccountId::from_u128(4), |a| {
            a.suspend(HEADQUARTERS_ID, SystemTime::UNIX_EPOCH)
        })
        .await
        .unwrap();
    let router = app_with(accounts().await, acceptances);

    let (status, body) = list_coaches(&router, "admin-web", "", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let statuses: Vec<(&str, &str)> = body["coaches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["coach_id"].as_str().unwrap(),
                c["acceptance_status"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        statuses,
        [
            ("00000000-0000-0000-0000-000000000004", "suspended"),
            ("00000000-0000-0000-0000-000000000005", "accepting"),
            ("00000000-0000-0000-0000-000000000002", "accepting"),
        ]
    );

    let (status, body) = list_coaches(&router, "student-app", "", Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<&str> = body["coaches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["coach_id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["00000000-0000-0000-0000-000000000002"],
        "受付を止めたテニスのコーチ（4）は生徒の一覧に出ない"
    );
}

#[tokio::test]
async fn no_coaches_is_an_empty_list() {
    let router = app(InMemoryAccountRepository::default());
    let (status, body) = list_coaches(&router, "admin-web", "", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "coaches": [] }));
}

#[tokio::test]
async fn student_lists_coaches_of_own_sport_by_average_rating() {
    let router = app(accounts().await);
    let (status, body) = list_coaches(&router, "student-app", "", Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coaches": [
                {
                    "coach_id": "00000000-0000-0000-0000-000000000004",
                    "display_name": "いとう",
                    "sport": "tennis",
                    "review_count": 1,
                    "average_rating": 5.0,
                    "published_lesson_menu_count": 0,
                    "is_favorite": true,
                },
                {
                    "coach_id": "00000000-0000-0000-0000-000000000002",
                    "display_name": "たなか",
                    "sport": "tennis",
                    "review_count": 2,
                    "average_rating": 3.5,
                    "published_lesson_menu_count": 1,
                    "is_favorite": false,
                },
            ],
        }),
        "生徒のスポーツ（テニス）を提供するコーチだけ。非公開のメニューは数えない。お気に入りはその生徒の分だけ"
    );
}

#[tokio::test]
async fn student_filters_by_sport() {
    let router = app(accounts().await);
    let (status, body) = list_coaches(&router, "student-app", "?sport=soccer", Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coaches": [{
                "coach_id": "00000000-0000-0000-0000-000000000005",
                "display_name": "いとう",
                "sport": "soccer",
                "review_count": 1,
                "average_rating": 2.0,
                "published_lesson_menu_count": 1,
                "is_favorite": false,
            }],
        }),
        "ゴルフのメニューは数えない"
    );

    let (status, body) =
        list_coaches(&router, "student-app", "?sport=swimming", Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "coaches": [] }));
}

#[tokio::test]
async fn golf_or_unknown_sport_is_422_and_golf_student_gets_an_empty_list() {
    let router = app(accounts().await);
    for query in ["?sport=golf", "?sport=cricket"] {
        let (status, body) = list_coaches(&router, "student-app", query, Some(STUDENT)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed");
    }

    let (status, body) = list_coaches(&router, "student-app", "", Some(GOLF_STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "coaches": [] }), "ゴルフは golf-app の範囲");
}

#[tokio::test]
async fn student_without_account_is_401_when_sport_is_omitted() {
    let router = app(accounts().await);
    let (status, body) = list_coaches(&router, "student-app", "", Some("student:99")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn coach_is_403_and_no_token_is_401() {
    let router = app(accounts().await);
    for contract in ["admin-web", "student-app"] {
        let (status, body) = list_coaches(&router, contract, "", Some("coach:2")).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{contract}: {body}");
        assert_eq!(body["code"], "forbidden");

        let (status, body) = list_coaches(&router, contract, "", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{contract}: {body}");
        assert_eq!(body["code"], "unauthorized");
    }
}
