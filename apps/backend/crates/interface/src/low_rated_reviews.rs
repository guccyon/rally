//! `/lesson-reviews/low`。admin-web の `GET /lesson-reviews/low`（`listLowRatedLessonReviews`。本部が横断で見る
//! 直近の低い評価の一覧）。本部だけが呼べる。しきい値と期間は domain の [`LowRatedReviewCriteria`] の値をそのまま載せる。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::account_id_uuid,
    lesson::lesson_completion_id_uuid,
    review::{
        LessonReviewRepository, ListLowRatedReviews, LowRatedReviewList,
        ReviewedCoachDisplayNameLookup, ReviewedLessonMenuLookup, ReviewerDisplayNameLookup,
        lesson_review_id_uuid,
    },
};
use axum::{Json, extract::State};
use domain::review::{LessonReview, LowRatedReviewCriteria};
use serde::Serialize;

use crate::{
    contract::date_time_to_contract,
    error::ApiError,
    reviews::{ReviewLessonMenuBody, review_lesson_menu_body},
};

/// `/lesson-reviews/low` のハンドラーが使うユースケース。
pub struct LowRatedReviewsState<R, M, D, S, C> {
    pub list_low_rated_reviews: ListLowRatedReviews<R, M, D, S, C>,
}

/// ハンドラーが共有する [`LowRatedReviewsState`]。
type Shared<R, M, D, S, C> = Arc<LowRatedReviewsState<R, M, D, S, C>>;

/// admin-web の `LowRatedLessonReviewList`。
#[derive(Serialize)]
pub struct LowRatedLessonReviewListBody {
    max_rating: u8,
    period_days: u32,
    reviews: Vec<LowRatedLessonReviewBody>,
}

/// admin-web の `LowRatedLessonReview`。表示名が引けなければ項目ごと載せない。
#[derive(Serialize)]
pub struct LowRatedLessonReviewBody {
    review_id: String,
    lesson_id: String,
    coach_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    student_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
    rating: u8,
    comment: String,
    submitted_at: String,
    lesson_menu: Option<ReviewLessonMenuBody>,
}

/// `GET /lesson-reviews/low`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。
pub async fn list_low_rated_reviews<R, M, D, S, C>(
    State(state): State<Shared<R, M, D, S, C>>,
) -> Result<Json<LowRatedLessonReviewListBody>, ApiError>
where
    R: LessonReviewRepository,
    M: ReviewedLessonMenuLookup,
    D: ReviewedCoachDisplayNameLookup,
    S: ReviewerDisplayNameLookup,
    C: Clock,
{
    let list = state
        .list_low_rated_reviews
        .execute()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(LowRatedLessonReviewListBody {
        max_rating: LowRatedReviewCriteria::MAX_RATING,
        period_days: LowRatedReviewCriteria::WINDOW_DAYS,
        reviews: list
            .reviews
            .iter()
            .map(|review| low_rated_review_body(review, &list))
            .collect::<Result<_, _>>()?,
    }))
}

/// `list` の評価 `review` 1 件。メニュー・表示名は `list` にあれば載せる。
fn low_rated_review_body(
    review: &LessonReview,
    list: &LowRatedReviewList,
) -> Result<LowRatedLessonReviewBody, ApiError> {
    Ok(LowRatedLessonReviewBody {
        review_id: lesson_review_id_uuid::format(review.id()),
        lesson_id: lesson_completion_id_uuid::format(review.lesson_id()),
        coach_id: account_id_uuid::format(review.coach_id()),
        coach_display_name: list
            .coach_display_names
            .get(&review.coach_id())
            .map(|name| name.as_str().to_string()),
        student_id: account_id_uuid::format(review.student_id()),
        student_display_name: list
            .student_display_names
            .get(&review.student_id())
            .map(|name| name.as_str().to_string()),
        rating: review.rating().value(),
        comment: review.comment().as_str().to_string(),
        submitted_at: date_time_to_contract(review.submitted_at())?,
        lesson_menu: list
            .lesson_menus
            .get(&review.lesson_id())
            .map(review_lesson_menu_body),
    })
}
