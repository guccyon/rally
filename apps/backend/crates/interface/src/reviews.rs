//! レッスンの評価。student-app の `POST /lesson-reviews`（`submitLessonReview`。生徒だけ）と、admin-web・student-app の
//! `GET /coaches/{coach_id}/review-summary`（`getCoachReviewSummary`。本部と生徒はどのコーチの分も、コーチは自分の分だけ）と、
//! `GET /coaches/{coach_id}/reviews`（`listCoachReviews`。コーチが受けた評価の一覧。読める範囲は件数と平均と同じ）。
//! 評価の一覧には、評価した生徒を特定できる項目を載せない（[`CoachReviewBody`] に `student_id` がない）。
//! 自分が生徒でないレッスンには、レッスンがあることも知らせない（404）。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    identity::account_id_uuid,
    lesson::lesson_id_uuid,
    review::{
        CompletedLessonLookup, GetCoachReviewSummary, LessonReviewRepository, ListCoachReviews,
        SubmitLessonReview, SubmitLessonReviewCommand, SubmitLessonReviewError,
        lesson_review_id_uuid,
    },
};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
};
use domain::{
    DomainError,
    identity::Role,
    review::{CoachReviewSummary, LessonReview, LessonReviewComment, Rating},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedAccount, contract::date_time_to_contract, error::ApiError, message,
};

/// レッスンの評価のハンドラーが使うユースケース。
pub struct ReviewsState<L, R, A, C> {
    pub submit_lesson_review: SubmitLessonReview<L, R, A, C>,
    pub get_coach_review_summary: GetCoachReviewSummary<R>,
    pub list_coach_reviews: ListCoachReviews<R>,
}

/// ハンドラーが共有する [`ReviewsState`]。
type Shared<L, R, A, C> = Arc<ReviewsState<L, R, A, C>>;

/// student-app の `SubmitLessonReviewRequest`。
#[derive(Deserialize)]
pub struct SubmitLessonReviewRequest {
    lesson_id: String,
    rating: i64,
    comment: String,
}

/// student-app の `LessonReview`。
#[derive(Serialize)]
pub struct LessonReviewBody {
    review_id: String,
    lesson_id: String,
    student_id: String,
    coach_id: String,
    rating: u8,
    comment: String,
    submitted_at: String,
}

/// admin-web・student-app の `CoachReviewSummary`。
#[derive(Serialize)]
pub struct CoachReviewSummaryBody {
    coach_id: String,
    review_count: u32,
    average_rating: Option<f64>,
}

/// admin-web・student-app の `CoachReview`。評価した生徒（`student_id`）は載せない。
#[derive(Serialize)]
pub struct CoachReviewBody {
    review_id: String,
    lesson_id: String,
    rating: u8,
    comment: String,
    submitted_at: String,
}

/// admin-web・student-app の `CoachReviewList`。
#[derive(Serialize)]
pub struct CoachReviewListBody {
    coach_id: String,
    reviews: Vec<CoachReviewBody>,
}

/// `POST /lesson-reviews`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。評価するのはトークンのアカウント。
pub async fn submit_lesson_review<L, R, A, C>(
    State(state): State<Shared<L, R, A, C>>,
    account: AuthenticatedAccount,
    request: Result<Json<SubmitLessonReviewRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<LessonReviewBody>), ApiError>
where
    L: CompletedLessonLookup,
    R: LessonReviewRepository,
    A: AnalyticsSink,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let command = SubmitLessonReviewCommand {
        account_id: account.account_id(),
        lesson_id: lesson_id_uuid::parse(&request.lesson_id)
            .map_err(|_| ApiError::validation_failed("lesson_id が UUID でない"))?,
        rating: Rating::new(request.rating)?,
        comment: LessonReviewComment::new(request.comment)?,
    };

    let review =
        state
            .submit_lesson_review
            .execute(command)
            .await
            .map_err(|error| match error {
                // 自分が生徒でないレッスンは、あることも知らせない
                SubmitLessonReviewError::LessonNotFound
                | SubmitLessonReviewError::Invalid(DomainError::NotTheLessonStudent) => {
                    ApiError::not_found("完了したレッスンがない")
                }
                SubmitLessonReviewError::Invalid(error @ DomainError::LessonAlreadyReviewed) => {
                    ApiError::conflict("lesson_already_reviewed", message::domain_error(&error))
                }
                SubmitLessonReviewError::Invalid(error) => ApiError::internal(error),
                SubmitLessonReviewError::Unavailable(error) => ApiError::internal(error),
            })?;
    Ok((StatusCode::CREATED, Json(lesson_review_body(&review)?)))
}

/// `GET /coaches/{coach_id}/review-summary`（admin-web・student-app）。本部・コーチ・生徒が呼べ（役割は `require_role` の
/// 層が確かめる）、コーチは自分の分だけを読める。本部と生徒はどのコーチの分も読める。
pub async fn get_coach_review_summary<L, R, A, C>(
    State(state): State<Shared<L, R, A, C>>,
    account: AuthenticatedAccount,
    Path(coach_id): Path<String>,
) -> Result<Json<CoachReviewSummaryBody>, ApiError>
where
    L: CompletedLessonLookup,
    R: LessonReviewRepository,
    A: AnalyticsSink,
    C: Clock,
{
    // 契約の getCoachReviewSummary に 422 はないので、UUID でない ID はないものとして扱う
    let coach_id =
        account_id_uuid::parse(&coach_id).map_err(|_| ApiError::not_found("コーチがない"))?;
    if account.role() == Role::Coach && account.account_id() != coach_id {
        return Err(ApiError::forbidden("コーチは自分の評価だけを読める"));
    }

    let summary = state
        .get_coach_review_summary
        .execute(coach_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(coach_review_summary_body(&summary)))
}

/// `GET /coaches/{coach_id}/reviews`（admin-web・student-app）。本部・コーチ・生徒が呼べ（役割は `require_role` の層が
/// 確かめる）、コーチは自分の分だけを読める。本部と生徒はどのコーチの分も読める。
pub async fn list_coach_reviews<L, R, A, C>(
    State(state): State<Shared<L, R, A, C>>,
    account: AuthenticatedAccount,
    Path(coach_id): Path<String>,
) -> Result<Json<CoachReviewListBody>, ApiError>
where
    L: CompletedLessonLookup,
    R: LessonReviewRepository,
    A: AnalyticsSink,
    C: Clock,
{
    // 契約の listCoachReviews に 422 はないので、UUID でない ID はないものとして扱う
    let coach_id =
        account_id_uuid::parse(&coach_id).map_err(|_| ApiError::not_found("コーチがない"))?;
    if account.role() == Role::Coach && account.account_id() != coach_id {
        return Err(ApiError::forbidden("コーチは自分の評価だけを読める"));
    }

    let reviews = state
        .list_coach_reviews
        .execute(coach_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(CoachReviewListBody {
        coach_id: account_id_uuid::format(coach_id),
        reviews: reviews
            .iter()
            .map(coach_review_body)
            .collect::<Result<_, _>>()?,
    }))
}

fn lesson_review_body(review: &LessonReview) -> Result<LessonReviewBody, ApiError> {
    Ok(LessonReviewBody {
        review_id: lesson_review_id_uuid::format(review.id()),
        lesson_id: lesson_id_uuid::format(review.lesson_id()),
        student_id: account_id_uuid::format(review.student_id()),
        coach_id: account_id_uuid::format(review.coach_id()),
        rating: review.rating().value(),
        comment: review.comment().as_str().to_string(),
        submitted_at: date_time_to_contract(review.submitted_at())?,
    })
}

fn coach_review_summary_body(summary: &CoachReviewSummary) -> CoachReviewSummaryBody {
    CoachReviewSummaryBody {
        coach_id: account_id_uuid::format(summary.coach_id()),
        review_count: summary.review_count(),
        average_rating: summary.average_rating(),
    }
}

fn coach_review_body(review: &LessonReview) -> Result<CoachReviewBody, ApiError> {
    Ok(CoachReviewBody {
        review_id: lesson_review_id_uuid::format(review.id()),
        lesson_id: lesson_id_uuid::format(review.lesson_id()),
        rating: review.rating().value(),
        comment: review.comment().as_str().to_string(),
        submitted_at: date_time_to_contract(review.submitted_at())?,
    })
}
