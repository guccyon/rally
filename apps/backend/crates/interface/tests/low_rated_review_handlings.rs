//! `PUT /lesson-reviews/{review_id}/handling`（admin-web の `markLowRatedLessonReviewHandled`）の統合テスト。本部が低い
//! 評価に対応済みの印（付けた本部のアカウントと時刻）を付け、2 回目は何も変えずに最初の印を返すことを OpenAPI のスキーマで
//! 検査する。点が 3 以上の評価は 409 `lesson_review_not_low_rated`、ない評価・UUID でない ID は 404、コーチ・生徒は 403、
//! トークンがなければ 401。

use std::time::{Duration, SystemTime};

use application::{
    Unavailable,
    identity::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier},
    in_memory::{FixedClock, InMemoryLessonReviewRepository},
    review::{InsertReviewError, LessonReviewRepository, MarkLowRatedReviewHandled},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    identity::{AccountId, Role},
    lesson::LessonCompletionId,
    review::{
        LessonReview, LessonReviewComment, LessonReviewId, LessonReviewReply,
        LowRatedReviewHandling, Rating,
    },
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
const OTHER_HEADQUARTERS: &str = "headquarters:9";
/// ★1 の評価
const LOW: &str = "00000000-0000-0000-0000-00000000000a";
/// ★3 の評価（低い評価でない）
const GOOD: &str = "00000000-0000-0000-0000-00000000000b";

/// 2026-10-31T00:00:00Z から `hours` 時間後。
fn at(hours: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800 + hours * 3600)
}

fn review(id: u128, rating: i64) -> LessonReview {
    LessonReview::restore(
        LessonReviewId::from_u128(id),
        LessonCompletionId::from_u128(id),
        AccountId::from_u128(4),
        AccountId::from_u128(2),
        Rating::new(rating).unwrap(),
        LessonReviewComment::new("説明が少なかった").unwrap(),
        at(0),
    )
}

fn reviews() -> InMemoryLessonReviewRepository {
    InMemoryLessonReviewRepository::with_reviews([review(10, 1), review(11, 3)])
}

fn router<R>(reviews: R, at: SystemTime) -> Router
where
    R: LessonReviewRepository + 'static,
{
    interface::low_rated_review_handlings_router(
        MarkLowRatedReviewHandled::new(reviews, FixedClock(at)),
        FakeTokens,
    )
}

/// `PUT /lesson-reviews/{review_id}/handling`（本文なし）。500 以外のレスポンスを admin-web の契約で検査する。
async fn mark(router: &Router, review_id: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::put(format!("/lesson-reviews/{review_id}/handling"));
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
            "put",
            "/lesson-reviews/{review_id}/handling",
            status,
            &body,
        );
    }
    (status, body)
}

#[tokio::test]
async fn headquarters_marks_a_low_review_handled_and_a_second_mark_returns_the_first() {
    let reviews = reviews();

    let (status, body) = mark(&router(reviews.clone(), at(1)), LOW, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let first = json!({
        "handled_by": "00000000-0000-0000-0000-000000000001",
        "handled_at": "2026-10-31T01:00:00Z",
    });
    assert_eq!(body, first);

    // 後の時刻に別の本部のアカウントが呼んでも、最初の印のまま
    let (status, body) = mark(
        &router(reviews.clone(), at(2)),
        LOW,
        Some(OTHER_HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, first);
    assert_eq!(
        reviews
            .find_by_id(LessonReviewId::from_u128(10))
            .await
            .unwrap()
            .unwrap()
            .handling(),
        Some(&LowRatedReviewHandling::new(AccountId::from_u128(1), at(1)))
    );
}

#[tokio::test]
async fn a_review_that_is_not_low_is_409_and_nothing_changes() {
    let reviews = reviews();
    let (status, body) = mark(&router(reviews.clone(), at(1)), GOOD, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "lesson_review_not_low_rated");
    assert_eq!(reviews.all(), vec![review(10, 1), review(11, 3)]);
}

#[tokio::test]
async fn an_unknown_review_and_an_id_that_is_not_a_uuid_are_404() {
    let router = router(reviews(), at(1));
    for review_id in ["00000000-0000-0000-0000-000000000063", "not-a-uuid"] {
        let (status, body) = mark(&router, review_id, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{review_id}: {body}");
        assert_eq!(body["code"], "not_found");
    }
}

#[tokio::test]
async fn coach_and_student_are_403_and_no_token_is_401() {
    let reviews = reviews();
    let router = router(reviews.clone(), at(1));
    for token in ["coach:2", "student:4"] {
        let (status, body) = mark(&router, LOW, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }

    let (status, body) = mark(&router, LOW, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
    assert_eq!(
        reviews.all(),
        vec![review(10, 1), review(11, 3)],
        "何も変えない"
    );
}

/// 評価の保存先が障害を返す偽物。
#[derive(Clone)]
struct BrokenReviews;

impl LessonReviewRepository for BrokenReviews {
    fn next_id(&self) -> LessonReviewId {
        LessonReviewId::from_u128(0)
    }

    async fn find_by_id(
        &self,
        _review_id: LessonReviewId,
    ) -> Result<Option<LessonReview>, Unavailable> {
        Err(Unavailable::new(
            "connection to postgres://rally:secret@db.internal:5432 refused",
        ))
    }

    async fn find_by_lesson(
        &self,
        _lesson_id: LessonCompletionId,
    ) -> Result<Option<LessonReview>, Unavailable> {
        unreachable!("印では読まない")
    }

    async fn insert(&self, _review: LessonReview) -> Result<(), InsertReviewError> {
        unreachable!("印では評価を保存しない")
    }

    async fn save_reply(
        &self,
        _review_id: LessonReviewId,
        _reply: &LessonReviewReply,
    ) -> Result<(), Unavailable> {
        unreachable!("印では返信を保存しない")
    }

    async fn save_handling(
        &self,
        _review_id: LessonReviewId,
        _handling: &LowRatedReviewHandling,
    ) -> Result<(), Unavailable> {
        unreachable!("読めなければ保存しない")
    }

    async fn find_by_coach(&self, _coach_id: AccountId) -> Result<Vec<LessonReview>, Unavailable> {
        unreachable!("印では読まない")
    }

    async fn find_submitted_since(
        &self,
        _since: SystemTime,
    ) -> Result<Vec<LessonReview>, Unavailable> {
        unreachable!("印では読まない")
    }
}

#[tokio::test]
async fn repository_failure_is_500_internal_without_the_reason() {
    // 契約は 500 の応答を書かないので、形は検査しない
    let (status, body) = mark(&router(BrokenReviews, at(1)), LOW, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["code"], "internal");
    assert!(!body.to_string().contains("secret"), "{body}");
}
