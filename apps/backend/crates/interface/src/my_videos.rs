//! `/videos/me`。student-app の `GET /videos/me`（`listMyVideos`。生徒が送った動画の一覧）。生徒だけが呼べる。

use std::sync::Arc;

use application::{
    async_lesson::{ListMyVideos, VideoCoachDisplayNameLookup, VideoRepository, video_id_uuid},
    identity::account_id_uuid,
};
use axum::{Json, extract::State};
use domain::identity::Sport;
use serde::Serialize;

use crate::{
    auth::AuthenticatedAccount,
    contract::{date_time_to_contract, sport_to_contract, video_review_status_to_contract},
    error::ApiError,
    videos::{VideoLessonMenuBody, video_lesson_menu_body},
};

/// `/videos/me` のハンドラーが使うユースケース。
pub struct MyVideosState<V, D> {
    pub list_my_videos: ListMyVideos<V, D>,
}

/// student-app の `MyVideoList`。
#[derive(Serialize)]
pub struct MyVideoListBody {
    videos: Vec<MyVideoBody>,
}

/// student-app の `MyVideo`。コーチの表示名は引けなければ載せない。返却された時刻は添削待ちなら null。送った時点の
/// レッスンメニューはメニューを選ばずに送った動画なら null。
#[derive(Serialize)]
pub struct MyVideoBody {
    video_id: String,
    coach_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    sport: &'static str,
    submitted_at: String,
    review_status: &'static str,
    returned_at: Option<String>,
    lesson_menu: Option<VideoLessonMenuBody>,
}

/// `GET /videos/me`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンのアカウントが
/// 送った動画。ゴルフの動画は golf-app の範囲（student-app の `Sport` にない）なので載せない。
pub async fn list_my_videos<V, D>(
    State(state): State<Arc<MyVideosState<V, D>>>,
    account: AuthenticatedAccount,
) -> Result<Json<MyVideoListBody>, ApiError>
where
    V: VideoRepository,
    D: VideoCoachDisplayNameLookup,
{
    let list = state
        .list_my_videos
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(MyVideoListBody {
        videos: list
            .videos
            .iter()
            .filter(|video| video.sport() != Sport::Golf)
            .map(|video| {
                Ok(MyVideoBody {
                    video_id: video_id_uuid::format(video.id()),
                    coach_id: account_id_uuid::format(video.coach_id()),
                    coach_display_name: list
                        .coaches
                        .get(&video.coach_id())
                        .map(|name| name.as_str().to_string()),
                    sport: sport_to_contract(video.sport()),
                    submitted_at: date_time_to_contract(video.submitted_at())?,
                    review_status: video_review_status_to_contract(video.status()),
                    returned_at: video
                        .review()
                        .map(|review| date_time_to_contract(review.returned_at()))
                        .transpose()?,
                    lesson_menu: video_lesson_menu_body(video),
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}
