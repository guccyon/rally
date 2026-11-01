//! `/video-reviews/pending`。admin-web の `GET /video-reviews/pending`（`listPendingVideoReviews`。本部が横断で見る
//! 返却を待っている動画添削の一覧）。本部だけが呼べる。クエリの `sport` があればそのスポーツの動画だけ（`Sport` の値で
//! なければ 422）、`min_elapsed_hours` があれば送付からの経過時間がその時間以上の動画だけ（0〜2147483647 の整数でなければ
//! 422）。経過時間の数え方と並べ方は domain の [`PendingVideoReviewFilter`]。

use std::sync::Arc;

use application::{
    async_lesson::{
        ListPendingVideoReviews, PendingVideoReviewList, VideoCoachDisplayNameLookup,
        VideoRepository, VideoStudentDisplayNameLookup, video_id_uuid,
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
    async_lesson::{PendingVideoReviewFilter, Video},
    identity::Role,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{TokenVerification, require_role},
    contract::{admin_web_sport, date_time_to_contract, sport_to_contract},
    error::ApiError,
};

/// 契約の `min_elapsed_hours` の上限（`maximum`。32 ビットの符号付き整数の最大）。
const MAX_MIN_ELAPSED_HOURS: u32 = i32::MAX as u32;

/// `/video-reviews/pending` のハンドラーが使うユースケース。
pub struct PendingVideoReviewsState<V, K, D, C> {
    pub list_pending_video_reviews: ListPendingVideoReviews<V, K, D, C>,
}

/// admin-web の `listPendingVideoReviews` のクエリ。
#[derive(Deserialize)]
pub struct ListPendingVideoReviewsParams {
    sport: Option<String>,
    min_elapsed_hours: Option<String>,
}

/// admin-web の `PendingVideoReviewList`。
#[derive(Serialize)]
pub struct PendingVideoReviewListBody {
    as_of: String,
    videos: Vec<PendingVideoReviewBody>,
}

/// admin-web の `PendingVideoReview`。表示名が引けなければ項目ごと載せない。
#[derive(Serialize)]
pub struct PendingVideoReviewBody {
    video_id: String,
    sport: &'static str,
    student_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
    coach_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    submitted_at: String,
    elapsed_hours: u64,
}

/// `GET /video-reviews/pending`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。
pub async fn list_pending_video_reviews<V, K, D, C>(
    State(state): State<Arc<PendingVideoReviewsState<V, K, D, C>>>,
    params: Result<Query<ListPendingVideoReviewsParams>, QueryRejection>,
) -> Result<Json<PendingVideoReviewListBody>, ApiError>
where
    V: VideoRepository,
    K: VideoCoachDisplayNameLookup,
    D: VideoStudentDisplayNameLookup,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let filter = PendingVideoReviewFilter {
        sport: params.sport.as_deref().map(admin_web_sport).transpose()?,
        min_elapsed_hours: params
            .min_elapsed_hours
            .as_deref()
            .map(min_elapsed_hours)
            .transpose()?
            .unwrap_or(0),
    };
    let list = state
        .list_pending_video_reviews
        .execute(filter)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(PendingVideoReviewListBody {
        as_of: date_time_to_contract(list.as_of)?,
        videos: list
            .videos
            .iter()
            .map(|video| pending_video_review_body(video, &list))
            .collect::<Result<_, _>>()?,
    }))
}

/// `min_elapsed_hours`（0〜[`MAX_MIN_ELAPSED_HOURS`] の整数）。
fn min_elapsed_hours(value: &str) -> Result<u32, ApiError> {
    value
        .parse::<u32>()
        .ok()
        .filter(|hours| *hours <= MAX_MIN_ELAPSED_HOURS)
        .ok_or_else(|| {
            ApiError::validation_failed(format!(
                "min_elapsed_hours は 0〜{MAX_MIN_ELAPSED_HOURS} の整数"
            ))
        })
}

/// `list` の動画 `video` 1 件。表示名は `list` にあれば載せ、経過時間は `list.as_of` までを数える。
fn pending_video_review_body(
    video: &Video,
    list: &PendingVideoReviewList,
) -> Result<PendingVideoReviewBody, ApiError> {
    Ok(PendingVideoReviewBody {
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
        elapsed_hours: PendingVideoReviewFilter::elapsed_hours(video, list.as_of),
    })
}

/// 本部が見る返却待ちの動画添削の一覧（admin-web の `GET /video-reviews/pending`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /video-reviews/pending` は本部だけ（コーチ・生徒は 403）。
pub fn pending_video_reviews_router<V, K, D, C, T>(
    list_pending_video_reviews: ListPendingVideoReviews<V, K, D, C>,
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
            "/video-reviews/pending",
            get(self::list_pending_video_reviews::<V, K, D, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(PendingVideoReviewsState {
            list_pending_video_reviews,
        }))
        .layer(TokenVerification::layer(tokens))
}
