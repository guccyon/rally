//! `PUT /lesson-reviews/{review_id}/reply`（admin-web の `replyToLessonReview`）の統合テスト。評価されたコーチが返信を
//! 付け、2 回目は上書きすることと、レスポンスを admin-web の契約で検査する。他のコーチ・本部・生徒は 403、ない評価・UUID で
//! ない ID は 404、空・長すぎる返信と形の違う本文は 422（何も変えない）、トークンがなければ 401。

use std::time::{Duration, SystemTime};

use application::{
    Unavailable,
    identity::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier},
    in_memory::{FixedClock, InMemoryLessonReviewRepository},
    review::{InsertReviewError, LessonReviewRepository, ReplyToLessonReview},
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
        LessonReviewReplyComment, Rating,
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

/// 評価されたコーチ
const COACH: &str = "coach:42";
const OTHER_COACH: &str = "coach:43";
/// 評価した生徒
const STUDENT: &str = "student:101";
const HEADQUARTERS: &str = "headquarters:1";
const REVIEW_ID: &str = "00000000-0000-0000-0000-000000000001";

/// 2026-10-26T00:00:00Z
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
}

/// 生徒 101 がコーチ 42 に付けた評価 1。
fn review() -> LessonReview {
    LessonReview::restore(
        LessonReviewId::from_u128(1),
        LessonCompletionId::from_u128(11),
        AccountId::from_u128(101),
        AccountId::from_u128(42),
        Rating::new(4).unwrap(),
        LessonReviewComment::new("よい").unwrap(),
        now() - Duration::from_secs(3600),
    )
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
        unreachable!("返信では読まない")
    }

    async fn insert(&self, _review: LessonReview) -> Result<(), InsertReviewError> {
        unreachable!("返信では評価を保存しない")
    }

    async fn save_reply(
        &self,
        _review_id: LessonReviewId,
        _reply: &LessonReviewReply,
    ) -> Result<(), Unavailable> {
        unreachable!("読めなければ保存しない")
    }

    async fn save_handling(
        &self,
        _review_id: LessonReviewId,
        _handling: &domain::review::LowRatedReviewHandling,
    ) -> Result<(), Unavailable> {
        unreachable!("返信では印を保存しない")
    }

    async fn find_by_coach(&self, _coach_id: AccountId) -> Result<Vec<LessonReview>, Unavailable> {
        unreachable!("返信では読まない")
    }

    async fn find_submitted_since(
        &self,
        _since: SystemTime,
    ) -> Result<Vec<LessonReview>, Unavailable> {
        unreachable!("返信では読まない")
    }
}

fn router<R>(reviews: R, at: SystemTime) -> Router
where
    R: LessonReviewRepository + 'static,
{
    interface::review_replies_router(
        ReplyToLessonReview::new(reviews, FixedClock(at)),
        FakeTokens,
    )
}

/// `PUT /lesson-reviews/{review_id}/reply`。500 以外のレスポンスを admin-web の契約で検査する。
async fn reply(
    router: &Router,
    review_id: &str,
    token: Option<&str>,
    body: &str,
) -> (StatusCode, Value) {
    let mut request = Request::put(format!("/lesson-reviews/{review_id}/reply"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    if status != StatusCode::INTERNAL_SERVER_ERROR {
        support::Contract::load("admin-web").assert_response(
            "put",
            "/lesson-reviews/{review_id}/reply",
            status,
            &body,
        );
    }
    (status, body)
}

fn comment(text: &str) -> String {
    json!({ "comment": text }).to_string()
}

#[tokio::test]
async fn the_reviewed_coach_replies_and_a_second_reply_overwrites_it() {
    let reviews = InMemoryLessonReviewRepository::with_reviews([review()]);

    let (status, body) = reply(
        &router(reviews.clone(), now()),
        REVIEW_ID,
        Some(COACH),
        &comment("ありがとうございます"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "comment": "ありがとうございます", "replied_at": "2026-10-26T00:00:00Z" })
    );

    let later = now() + Duration::from_secs(60);
    let (status, body) = reply(
        &router(reviews.clone(), later),
        REVIEW_ID,
        Some(COACH),
        &comment("次回はサーブも見ます"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "comment": "次回はサーブも見ます", "replied_at": "2026-10-26T00:01:00Z" })
    );
    // 評価は 1 件のままで、返信だけが置き換わる
    assert_eq!(
        reviews.all(),
        vec![review().with_reply(Some(LessonReviewReply::new(
            LessonReviewReplyComment::new("次回はサーブも見ます").unwrap(),
            later,
        )))]
    );
}

#[tokio::test]
async fn others_than_the_reviewed_coach_are_403_and_no_token_is_401() {
    let reviews = InMemoryLessonReviewRepository::with_reviews([review()]);
    let router = router(reviews.clone(), now());
    // 他のコーチはユースケースが、本部・生徒は役割の層が拒む
    for token in [OTHER_COACH, HEADQUARTERS, STUDENT] {
        let (status, body) = reply(&router, REVIEW_ID, Some(token), &comment("横取り")).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }

    let (status, body) = reply(&router, REVIEW_ID, None, &comment("ありがとう")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
    assert_eq!(reviews.all(), vec![review()], "何も変えない");
}

#[tokio::test]
async fn an_unknown_review_or_a_malformed_id_is_404() {
    let router = router(
        InMemoryLessonReviewRepository::with_reviews([review()]),
        now(),
    );
    for review_id in ["00000000-0000-0000-0000-000000000099", "not-a-uuid"] {
        let (status, body) = reply(&router, review_id, Some(COACH), &comment("ありがとう")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{review_id}: {body}");
        assert_eq!(body["code"], "not_found");
    }
}

#[tokio::test]
async fn invalid_replies_are_422_and_change_nothing() {
    let reviews = InMemoryLessonReviewRepository::with_reviews([review()]);
    let router = router(reviews.clone(), now());
    for (body, message) in [
        (comment(""), Some("評価への返信が空")),
        (comment("\u{3000} "), Some("評価への返信が空")),
        (
            comment(&"あ".repeat(201)),
            Some("評価への返信は 200 文字まで"),
        ),
        ("{}".to_string(), None),
        (json!({ "comment": 1 }).to_string(), None),
    ] {
        let (status, response) = reply(&router, REVIEW_ID, Some(COACH), &body).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{body}: {response}"
        );
        assert_eq!(response["code"], "validation_failed");
        if let Some(message) = message {
            assert_eq!(response["message"], message);
        }
    }
    // 200 文字ちょうどは通る
    let (status, body) = reply(&router, REVIEW_ID, Some(COACH), &comment(&"あ".repeat(200))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn repository_failure_is_500_internal_without_the_reason() {
    // 契約は 500 の応答を書かないので、形は検査しない
    let (status, body) = reply(
        &router(BrokenReviews, now()),
        REVIEW_ID,
        Some(COACH),
        &comment("ありがとう"),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["code"], "internal");
    assert!(!body.to_string().contains("secret"), "{body}");
}
