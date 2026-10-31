//! `GET /lesson-reviews/low`（admin-web の `listLowRatedLessonReviews`）の統合テスト。本部には直近 30 日の★2 以下の評価を
//! コーチを問わず新しい順に、コーチ・生徒の表示名とレッスンメニューとともに返すことを OpenAPI のスキーマで検査する。
//! コーチ・生徒は 403、トークンがなければ 401。この操作はパスパラメータもクエリも受けないので、404（見つからない）と
//! 422（規則違反）の応答はない（しきい値・期間の規則の境目は domain の `LowRatedReviewCriteria` のテストと、ここの
//! 一覧に載らない評価で確かめる）。

use std::time::{Duration, SystemTime};

use application::{
    Unavailable,
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{FixedClock, InMemoryAccountRepository, InMemoryLessonReviewRepository},
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
    review::{LessonReviewRepository, ListLowRatedReviews, ReviewedLessonMenuLookup},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::{LessonMenuId, LessonMenuName, Sport},
    identity::{Account, AccountId, Email, Role},
    lesson::LessonCompletionId,
    profile::DisplayName,
    review::{LessonReview, LessonReviewComment, LessonReviewId, Rating, ReviewLessonMenu},
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
const COACH: AccountId = AccountId::from_u128(2);
const OTHER_COACH: AccountId = AccountId::from_u128(3);
const STUDENT: AccountId = AccountId::from_u128(4);
/// アカウントのない生徒（表示名が引けない）
const UNKNOWN_STUDENT: AccountId = AccountId::from_u128(5);
const DAY: Duration = Duration::from_secs(24 * 3600);

/// 照会する時刻（2026-10-31T00:00:00Z）。
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
}

/// レッスン 10 だけがメニューを持つ。
struct FixedLessonMenus;

impl ReviewedLessonMenuLookup for FixedLessonMenus {
    async fn find_lesson_menu(
        &self,
        lesson_id: LessonCompletionId,
    ) -> Result<Option<ReviewLessonMenu>, Unavailable> {
        Ok((lesson_id.as_u128() == 10).then(|| {
            ReviewLessonMenu::new(
                LessonMenuId::from_u128(20),
                LessonMenuName::new("フォア改善").unwrap(),
            )
        }))
    }
}

/// 評価の保存先が障害を返す偽物。
#[derive(Clone)]
struct BrokenReviews;

impl LessonReviewRepository for BrokenReviews {
    fn next_id(&self) -> LessonReviewId {
        LessonReviewId::from_u128(0)
    }

    async fn find_by_lesson(
        &self,
        _lesson_id: LessonCompletionId,
    ) -> Result<Option<LessonReview>, Unavailable> {
        unreachable!("一覧では読まない")
    }

    async fn insert(
        &self,
        _review: LessonReview,
    ) -> Result<(), application::review::InsertReviewError> {
        unreachable!("一覧では保存しない")
    }

    async fn find_by_coach(&self, _coach_id: AccountId) -> Result<Vec<LessonReview>, Unavailable> {
        unreachable!("一覧では読まない")
    }

    async fn find_submitted_since(
        &self,
        _since: SystemTime,
    ) -> Result<Vec<LessonReview>, Unavailable> {
        Err(Unavailable::new(
            "connection to postgres://rally:secret@db.internal:5432 refused",
        ))
    }
}

fn account(id: AccountId, name: &str, role: Role) -> Account {
    Account::new(
        id,
        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
        DisplayName::new(name).unwrap(),
        role,
        if role == Role::Headquarters {
            vec![]
        } else {
            vec![Sport::Tennis]
        },
        SystemTime::UNIX_EPOCH,
    )
    .unwrap()
}

fn review(
    id: u128,
    coach_id: AccountId,
    student_id: AccountId,
    rating: i64,
    submitted_at: SystemTime,
) -> LessonReview {
    LessonReview::restore(
        LessonReviewId::from_u128(id),
        LessonCompletionId::from_u128(id),
        student_id,
        coach_id,
        Rating::new(rating).unwrap(),
        LessonReviewComment::new(format!("コメント{id}")).unwrap(),
        submitted_at,
    )
}

fn router<R>(accounts: InMemoryAccountRepository, reviews: R) -> Router
where
    R: LessonReviewRepository + 'static,
{
    interface::low_rated_reviews_router(
        ListLowRatedReviews::new(
            reviews,
            FixedLessonMenus,
            ProfileCoachDisplayNameLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts),
            FixedClock(now()),
        ),
        FakeTokens,
    )
}

async fn app() -> Router {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(AccountId::from_u128(1), "本部", Role::Headquarters),
        account(COACH, "たなかコーチ", Role::Coach),
        account(OTHER_COACH, "いとうコーチ", Role::Coach),
        account(STUDENT, "あおき", Role::Student),
    ] {
        accounts.insert(account).await.unwrap();
    }
    router(
        accounts,
        InMemoryLessonReviewRepository::with_reviews([
            review(10, COACH, STUDENT, 1, now() - 3 * DAY),
            review(11, OTHER_COACH, UNKNOWN_STUDENT, 2, now() - DAY),
            // ★3 は低い評価でない
            review(12, COACH, STUDENT, 3, now()),
            // 30 日より前は載らない
            review(
                13,
                COACH,
                STUDENT,
                1,
                now() - 30 * DAY - Duration::from_secs(1),
            ),
            // 30 日前ちょうどは載る
            review(14, OTHER_COACH, STUDENT, 2, now() - 30 * DAY),
        ]),
    )
}

/// `GET /lesson-reviews/low`。レスポンスを admin-web の契約で検査する。
async fn list(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/lesson-reviews/low");
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
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    if status != StatusCode::INTERNAL_SERVER_ERROR {
        support::Contract::load("admin-web").assert_response(
            "get",
            "/lesson-reviews/low",
            status,
            &body,
        );
    }
    (status, body)
}

#[tokio::test]
async fn headquarters_lists_low_reviews_of_the_last_thirty_days_newest_first() {
    let (status, body) = list(&app().await, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "max_rating": 2,
            "period_days": 30,
            "reviews": [
                {
                    "review_id": "00000000-0000-0000-0000-00000000000b",
                    "lesson_id": "00000000-0000-0000-0000-00000000000b",
                    "coach_id": "00000000-0000-0000-0000-000000000003",
                    "coach_display_name": "いとうコーチ",
                    // 生徒のアカウントが引けないので student_display_name は載らない
                    "student_id": "00000000-0000-0000-0000-000000000005",
                    "rating": 2,
                    "comment": "コメント11",
                    "submitted_at": "2026-10-30T00:00:00Z",
                    "lesson_menu": null,
                },
                {
                    "review_id": "00000000-0000-0000-0000-00000000000a",
                    "lesson_id": "00000000-0000-0000-0000-00000000000a",
                    "coach_id": "00000000-0000-0000-0000-000000000002",
                    "coach_display_name": "たなかコーチ",
                    "student_id": "00000000-0000-0000-0000-000000000004",
                    "student_display_name": "あおき",
                    "rating": 1,
                    "comment": "コメント10",
                    "submitted_at": "2026-10-28T00:00:00Z",
                    "lesson_menu": {
                        "lesson_menu_id": "00000000-0000-0000-0000-000000000014",
                        "name": "フォア改善",
                    },
                },
                {
                    "review_id": "00000000-0000-0000-0000-00000000000e",
                    "lesson_id": "00000000-0000-0000-0000-00000000000e",
                    "coach_id": "00000000-0000-0000-0000-000000000003",
                    "coach_display_name": "いとうコーチ",
                    "student_id": "00000000-0000-0000-0000-000000000004",
                    "student_display_name": "あおき",
                    "rating": 2,
                    "comment": "コメント14",
                    "submitted_at": "2026-10-01T00:00:00Z",
                    "lesson_menu": null,
                },
            ],
        }),
        "★3 以上（12）と 30 日より前（13）は載らない"
    );
}

#[tokio::test]
async fn no_low_reviews_is_an_empty_list() {
    let router = router(
        InMemoryAccountRepository::default(),
        InMemoryLessonReviewRepository::with_reviews([review(1, COACH, STUDENT, 5, now())]),
    );
    let (status, body) = list(&router, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "max_rating": 2, "period_days": 30, "reviews": [] })
    );
}

#[tokio::test]
async fn coach_and_student_are_403_and_no_token_is_401() {
    let router = app().await;
    for token in ["coach:2", "student:4"] {
        let (status, body) = list(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }

    let (status, body) = list(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn repository_failure_is_500_internal_without_the_reason() {
    // 契約は 500 の応答を書かないので、形は検査しない
    let (status, body) = list(
        &router(InMemoryAccountRepository::default(), BrokenReviews),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["code"], "internal");
    assert!(!body.to_string().contains("secret"), "{body}");
}
