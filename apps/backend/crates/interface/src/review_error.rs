//! review（評価）の不変条件の違反（[`ReviewError`]）を、OpenAPI の `Error` の応答に写す。
//!
//! 文言と HTTP の状態・`code` はここだけで決める（`/lesson-reviews`・`/lesson-reviews/{review_id}/reply`・
//! `/lesson-reviews/{review_id}/handling` のハンドラーは
//! 変種を個別に拾わない）。
//!
//! - 評価済み（`LessonAlreadyReviewed`）は 409 `lesson_already_reviewed`
//! - レッスンの生徒でない（`NotTheLessonStudent`）は、レッスンがあることも知らせない 404 `not_found`（ないレッスンと同じ）
//! - 点・一言の違反（`RatingOutOfRange`・`EmptyLessonReviewComment`・`LessonReviewCommentTooLong`）は 422
//!   `validation_failed`
//! - 評価されたコーチでない（`NotTheReviewedCoach`）は 403 `forbidden`（評価はコーチが受けた評価の一覧で誰でも読めるので、
//!   あることは隠さない）
//! - 返信の文の違反（`EmptyLessonReviewReply`・`LessonReviewReplyTooLong`）は 422 `validation_failed`
//! - 低い評価でない評価への本部の対応済みの印（`NotLowRated`）は 409 `lesson_review_not_low_rated`

use domain::review::ReviewError;

use crate::error::ApiError;

/// ない完了したレッスン（自分が生徒でないレッスンも同じ）の 404。
pub(crate) fn lesson_not_found() -> ApiError {
    ApiError::not_found("完了したレッスンがない")
}

impl From<ReviewError> for ApiError {
    fn from(error: ReviewError) -> Self {
        match error {
            ReviewError::LessonAlreadyReviewed => {
                Self::conflict("lesson_already_reviewed", "このレッスンは評価済み")
            }
            ReviewError::NotTheLessonStudent => lesson_not_found(),
            ReviewError::RatingOutOfRange { min, max } => {
                Self::validation_failed(format!("評価の点は {min}〜{max} の整数"))
            }
            ReviewError::EmptyLessonReviewComment => Self::validation_failed("評価の一言が空"),
            ReviewError::LessonReviewCommentTooLong { max } => {
                Self::validation_failed(format!("評価の一言は {max} 文字まで"))
            }
            ReviewError::NotTheReviewedCoach => {
                Self::forbidden("評価されたコーチだけが評価に返信できる")
            }
            ReviewError::EmptyLessonReviewReply => Self::validation_failed("評価への返信が空"),
            ReviewError::LessonReviewReplyTooLong { max } => {
                Self::validation_failed(format!("評価への返信は {max} 文字まで"))
            }
            ReviewError::NotLowRated => Self::conflict(
                "lesson_review_not_low_rated",
                "低い評価でないので対応済みにできない",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;

    #[test]
    fn maps_each_review_error_to_a_status_and_code() {
        for (error, status, code, message) in [
            (
                ReviewError::LessonAlreadyReviewed,
                StatusCode::CONFLICT,
                "lesson_already_reviewed",
                "このレッスンは評価済み",
            ),
            (
                ReviewError::NotTheLessonStudent,
                StatusCode::NOT_FOUND,
                "not_found",
                "完了したレッスンがない",
            ),
            (
                ReviewError::RatingOutOfRange { min: 1, max: 5 },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "評価の点は 1〜5 の整数",
            ),
            (
                ReviewError::EmptyLessonReviewComment,
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "評価の一言が空",
            ),
            (
                ReviewError::LessonReviewCommentTooLong { max: 200 },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "評価の一言は 200 文字まで",
            ),
            (
                ReviewError::NotTheReviewedCoach,
                StatusCode::FORBIDDEN,
                "forbidden",
                "評価されたコーチだけが評価に返信できる",
            ),
            (
                ReviewError::EmptyLessonReviewReply,
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "評価への返信が空",
            ),
            (
                ReviewError::LessonReviewReplyTooLong { max: 200 },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "評価への返信は 200 文字まで",
            ),
            (
                ReviewError::NotLowRated,
                StatusCode::CONFLICT,
                "lesson_review_not_low_rated",
                "低い評価でないので対応済みにできない",
            ),
        ] {
            let api_error = ApiError::from(error.clone());
            assert_eq!(
                (api_error.status(), api_error.code(), api_error.message()),
                (status, code, message),
                "{error:?}"
            );
        }
    }
}
