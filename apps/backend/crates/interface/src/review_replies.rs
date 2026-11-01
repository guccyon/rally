//! 評価への返信。admin-web の `PUT /lesson-reviews/{review_id}/reply`（`replyToLessonReview`。コーチだけ）。
//! 返信できるのは評価されたコーチだけで、他のコーチは 403。2 回目は文と時刻を上書きする。
//! 返信は `listCoachReviews`・`listLowRatedLessonReviews` の評価 1 件ごとに載る（[`crate::reviews`]・[`crate::low_rated_reviews`]）。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::TokenVerifier,
    review::{
        LessonReviewRepository, ReplyToLessonReview, ReplyToLessonReviewCommand,
        ReplyToLessonReviewError, lesson_review_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    routing::put,
};
use domain::{identity::Role, review::LessonReviewReplyComment};
use serde::Deserialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    error::ApiError,
    reviews::{LessonReviewReplyBody, lesson_review_reply_body},
};

/// `/lesson-reviews/{review_id}/reply` のハンドラーが使うユースケース。
pub struct ReviewRepliesState<R, C> {
    pub reply_to_lesson_review: ReplyToLessonReview<R, C>,
}

/// ハンドラーが共有する [`ReviewRepliesState`]。
type Shared<R, C> = Arc<ReviewRepliesState<R, C>>;

/// admin-web の `ReplyToLessonReviewRequest`。
#[derive(Deserialize)]
pub struct ReplyToLessonReviewRequest {
    comment: String,
}

/// `PUT /lesson-reviews/{review_id}/reply`（admin-web）。コーチだけが呼べ（役割は `require_role` の層が確かめる）、
/// 返信するのはトークンのアカウント。評価されたコーチかはユースケース（domain）が確かめる。
pub async fn reply_to_lesson_review<R, C>(
    State(state): State<Shared<R, C>>,
    account: AuthenticatedAccount,
    Path(review_id): Path<String>,
    request: Result<Json<ReplyToLessonReviewRequest>, JsonRejection>,
) -> Result<Json<LessonReviewReplyBody>, ApiError>
where
    R: LessonReviewRepository,
    C: Clock,
{
    // UUID でない ID はないものとして扱う（本文より先に確かめる）
    let review_id = lesson_review_id_uuid::parse(&review_id).map_err(|_| review_not_found())?;
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let command = ReplyToLessonReviewCommand {
        account_id: account.account_id(),
        review_id,
        comment: LessonReviewReplyComment::new(request.comment)?,
    };

    let review = state
        .reply_to_lesson_review
        .execute(command)
        .await
        .map_err(|error| match error {
            ReplyToLessonReviewError::ReviewNotFound => review_not_found(),
            ReplyToLessonReviewError::Invalid(error) => ApiError::from(error),
            ReplyToLessonReviewError::Unavailable(error) => ApiError::internal(error),
        })?;
    let reply = review
        .reply()
        .ok_or_else(|| ApiError::internal("返信を付けた評価に返信がない"))?;
    Ok(Json(lesson_review_reply_body(reply)?))
}

fn review_not_found() -> ApiError {
    ApiError::not_found("評価がない")
}

/// 評価への返信（admin-web の `PUT /lesson-reviews/{review_id}/reply`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// コーチだけ（本部・生徒は 403）で、返信できるのは評価されたコーチだけ（他のコーチも 403。ユースケースが確かめる）。
pub fn review_replies_router<R, C, T>(
    reply_to_lesson_review: ReplyToLessonReview<R, C>,
    tokens: T,
) -> Router
where
    R: LessonReviewRepository + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/lesson-reviews/{review_id}/reply",
            put(self::reply_to_lesson_review::<R, C>),
        )
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(ReviewRepliesState {
            reply_to_lesson_review,
        }))
        .layer(TokenVerification::layer(tokens))
}
