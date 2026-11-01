//! `/lesson-reviews/low`。admin-web の `GET /lesson-reviews/low`（`listLowRatedLessonReviews`。本部が横断で見る
//! 直近の低い評価の一覧）。本部だけが呼べる。クエリの `sport` があればそのスポーツのレッスンの評価だけ（`Sport` の値で
//! なければ 422）。`handled` があれば本部の対応済みの印の有無がそれと同じ評価だけ（`true` / `false` でなければ 422）。
//! 各評価には評価されたコーチの返信と、本部の対応済みの印を載せる（なければ null）。しきい値と期間は domain の [`LowRatedReviewCriteria`] の値をそのまま載せる。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    lesson::lesson_completion_id_uuid,
    review::{
        LessonReviewRepository, ListLowRatedReviews, LowRatedReviewList,
        ReviewedCoachDisplayNameLookup, ReviewedLessonMenuLookup, ReviewedLessonSportLookup,
        ReviewerDisplayNameLookup, lesson_review_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::{
    identity::Role,
    review::{LessonReview, LowRatedReviewCriteria},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{TokenVerification, require_role},
    contract::{admin_web_sport, date_time_to_contract},
    error::ApiError,
    low_rated_review_handlings::{LowRatedReviewHandlingBody, low_rated_review_handling_body},
    reviews::{
        LessonReviewReplyBody, ReviewLessonMenuBody, lesson_review_reply_body,
        review_lesson_menu_body,
    },
};

/// `/lesson-reviews/low` のハンドラーが使うユースケース。
pub struct LowRatedReviewsState<R, M, P, D, S, C> {
    pub list_low_rated_reviews: ListLowRatedReviews<R, M, P, D, S, C>,
}

/// ハンドラーが共有する [`LowRatedReviewsState`]。
type Shared<R, M, P, D, S, C> = Arc<LowRatedReviewsState<R, M, P, D, S, C>>;

/// admin-web の `listLowRatedLessonReviews` のクエリ。
#[derive(Deserialize)]
pub struct ListLowRatedReviewsParams {
    sport: Option<String>,
    handled: Option<bool>,
}

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
    reply: Option<LessonReviewReplyBody>,
    handling: Option<LowRatedReviewHandlingBody>,
}

/// `GET /lesson-reviews/low`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。
/// `sport` があればそのスポーツのレッスンの評価だけを返す（ゴルフも受け付ける）。`handled` があれば対応済みの印の
/// 有無がそれと同じ評価だけ（`false` で未対応だけ）。
pub async fn list_low_rated_reviews<R, M, P, D, S, C>(
    State(state): State<Shared<R, M, P, D, S, C>>,
    params: Result<Query<ListLowRatedReviewsParams>, QueryRejection>,
) -> Result<Json<LowRatedLessonReviewListBody>, ApiError>
where
    R: LessonReviewRepository,
    M: ReviewedLessonMenuLookup,
    P: ReviewedLessonSportLookup,
    D: ReviewedCoachDisplayNameLookup,
    S: ReviewerDisplayNameLookup,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let sport = params.sport.as_deref().map(admin_web_sport).transpose()?;
    let list = state
        .list_low_rated_reviews
        .execute(sport, params.handled)
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
        reply: review.reply().map(lesson_review_reply_body).transpose()?,
        handling: review
            .handling()
            .map(low_rated_review_handling_body)
            .transpose()?,
    })
}

/// 本部が見る低い評価の一覧（admin-web の `GET /lesson-reviews/low`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /lesson-reviews/low` は本部だけ（コーチ・生徒は 403）。同じ接頭辞の `POST /lesson-reviews`（評価）は
/// [`crate::reviews_router`] で、生徒だけ。対応済みの印を付ける `PUT /lesson-reviews/{review_id}/handling` は
/// [`crate::low_rated_review_handlings_router`]。
pub fn low_rated_reviews_router<R, M, P, D, S, C, T>(
    list_low_rated_reviews: ListLowRatedReviews<R, M, P, D, S, C>,
    tokens: T,
) -> Router
where
    R: LessonReviewRepository + 'static,
    M: ReviewedLessonMenuLookup + 'static,
    P: ReviewedLessonSportLookup + 'static,
    D: ReviewedCoachDisplayNameLookup + 'static,
    S: ReviewerDisplayNameLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/lesson-reviews/low",
            get(self::list_low_rated_reviews::<R, M, P, D, S, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(LowRatedReviewsState {
            list_low_rated_reviews,
        }))
        .layer(TokenVerification::layer(tokens))
}
