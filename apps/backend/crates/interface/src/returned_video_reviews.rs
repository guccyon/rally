//! `/video-reviews/returned`。admin-web の `GET /video-reviews/returned`（`listReturnedVideoReviews`。本部が横断で見る
//! 返した動画添削の記録）。本部だけが呼べる。クエリの `from`・`to` は返却の日付（日本時間。`YYYY-MM-DD` でなければ 422）、
//! `coach_id` があればそのコーチの動画だけ（UUID でなければ 422）。期間の規則・かかった時間の数え方・並べ方は domain の
//! [`ReturnedVideoReviewFilter`]（期間の違反も 422 `validation_failed`）。

use std::sync::Arc;

use application::{
    async_lesson::{
        ListReturnedVideoReviews, ListReturnedVideoReviewsError, ListReturnedVideoReviewsQuery,
        ReturnedVideoReviewList, VideoCoachDisplayNameLookup, VideoRepository,
        VideoStudentDisplayNameLookup, video_id_uuid,
    },
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::{
    async_lesson::{ReturnedVideoReviewFilter, Video},
    identity::Role,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{TokenVerification, require_role},
    contract::{date_from_contract, date_time_to_contract, date_to_contract, sport_to_contract},
    error::ApiError,
};

/// `/video-reviews/returned` のハンドラーが使うユースケース。
pub struct ReturnedVideoReviewsState<V, K, D, C> {
    pub list_returned_video_reviews: ListReturnedVideoReviews<V, K, D, C>,
}

/// admin-web の `listReturnedVideoReviews` のクエリ。
#[derive(Deserialize)]
pub struct ListReturnedVideoReviewsParams {
    from: Option<String>,
    to: Option<String>,
    coach_id: Option<String>,
}

/// admin-web の `ReturnedVideoReviewList`。
#[derive(Serialize)]
pub struct ReturnedVideoReviewListBody {
    from: String,
    to: String,
    reviews: Vec<ReturnedVideoReviewBody>,
}

/// admin-web の `ReturnedVideoReview`。表示名が引けなければ項目ごと載せない。
#[derive(Serialize)]
pub struct ReturnedVideoReviewBody {
    video_id: String,
    sport: &'static str,
    student_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
    coach_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    submitted_at: String,
    returned_at: String,
    lead_time_hours: u64,
}

/// `GET /video-reviews/returned`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。
pub async fn list_returned_video_reviews<V, K, D, C>(
    State(state): State<Arc<ReturnedVideoReviewsState<V, K, D, C>>>,
    params: Result<Query<ListReturnedVideoReviewsParams>, QueryRejection>,
) -> Result<Json<ReturnedVideoReviewListBody>, ApiError>
where
    V: VideoRepository,
    K: VideoCoachDisplayNameLookup,
    D: VideoStudentDisplayNameLookup,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let query = ListReturnedVideoReviewsQuery {
        from: params
            .from
            .as_deref()
            .map(|value| date_from_contract("from", value))
            .transpose()?,
        to: params
            .to
            .as_deref()
            .map(|value| date_from_contract("to", value))
            .transpose()?,
        coach_id: params
            .coach_id
            .as_deref()
            .map(|id| {
                account_id_uuid::parse(id)
                    .map_err(|_| ApiError::validation_failed("coach_id が UUID でない"))
            })
            .transpose()?,
    };
    let list = state
        .list_returned_video_reviews
        .execute(query)
        .await
        .map_err(|error| match error {
            ListReturnedVideoReviewsError::Invalid(error) => ApiError::from(error),
            ListReturnedVideoReviewsError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(ReturnedVideoReviewListBody {
        from: date_to_contract(list.from),
        to: date_to_contract(list.to),
        reviews: list
            .videos
            .iter()
            .map(|video| returned_video_review_body(video, &list))
            .collect::<Result<_, _>>()?,
    }))
}

/// `list` の返却済みの動画 `video` 1 件。表示名は `list` にあれば載せる。
fn returned_video_review_body(
    video: &Video,
    list: &ReturnedVideoReviewList,
) -> Result<ReturnedVideoReviewBody, ApiError> {
    let (Some(review), Some(lead_time_hours)) =
        (video.review(), ReturnedVideoReviewFilter::lead_hours(video))
    else {
        return Err(ApiError::internal(
            "返した動画添削の記録に返却していない動画がある",
        ));
    };
    Ok(ReturnedVideoReviewBody {
        video_id: video_id_uuid::format(video.id()),
        sport: sport_to_contract(video.sport()),
        student_id: account_id_uuid::format(video.student_id()),
        student_display_name: list
            .students
            .get(&video.student_id())
            .map(|name| name.as_str().to_string()),
        coach_id: account_id_uuid::format(video.coach_id()),
        coach_display_name: list
            .coaches
            .get(&video.coach_id())
            .map(|name| name.as_str().to_string()),
        submitted_at: date_time_to_contract(video.submitted_at())?,
        returned_at: date_time_to_contract(review.returned_at())?,
        lead_time_hours,
    })
}

/// 本部が見る返した動画添削の記録（admin-web の `GET /video-reviews/returned`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /video-reviews/returned` は本部だけ（コーチ・生徒は 403）。
pub fn returned_video_reviews_router<V, K, D, C, T>(
    list_returned_video_reviews: ListReturnedVideoReviews<V, K, D, C>,
    tokens: T,
) -> Router
where
    V: VideoRepository + 'static,
    K: VideoCoachDisplayNameLookup + 'static,
    D: VideoStudentDisplayNameLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/video-reviews/returned",
            get(self::list_returned_video_reviews::<V, K, D, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(ReturnedVideoReviewsState {
            list_returned_video_reviews,
        }))
        .layer(TokenVerification::layer(tokens))
}
