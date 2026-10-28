//! `/coaches/me/videos`。admin-web の `GET /coaches/me/videos`（`listMyCoachVideos`。コーチに届いた動画の一覧）。
//! コーチだけが呼べる。

use std::sync::Arc;

use application::async_lesson::{
    ListCoachVideos, VideoRepository, VideoStudentDisplayNameLookup, video_id_uuid,
};
use axum::{Json, extract::State};
use serde::Serialize;

use crate::{
    auth::AuthenticatedAccount,
    contract::{date_time_to_contract, video_review_status_to_contract},
    error::ApiError,
    videos::{AiFeedbackBody, ai_feedback_body},
};

/// `/coaches/me/videos` のハンドラーが使うユースケース。
pub struct CoachVideosState<V, D> {
    pub list_coach_videos: ListCoachVideos<V, D>,
}

/// admin-web の `CoachVideoList`。
#[derive(Serialize)]
pub struct CoachVideoListBody {
    videos: Vec<CoachVideoBody>,
}

/// admin-web の `CoachVideo`。生徒の表示名は引けなければ、返却した時刻は添削待ちなら載せない。AI の一次フィードバックは
/// まだなければ null。
#[derive(Serialize)]
pub struct CoachVideoBody {
    video_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
    submitted_at: String,
    review_status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    returned_at: Option<String>,
    ai_feedback: Option<AiFeedbackBody>,
}

/// `GET /coaches/me/videos`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンの
/// アカウントに送られた動画。
pub async fn list_my_coach_videos<V, D>(
    State(state): State<Arc<CoachVideosState<V, D>>>,
    account: AuthenticatedAccount,
) -> Result<Json<CoachVideoListBody>, ApiError>
where
    V: VideoRepository,
    D: VideoStudentDisplayNameLookup,
{
    let list = state
        .list_coach_videos
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(CoachVideoListBody {
        videos: list
            .videos
            .iter()
            .map(|video| {
                Ok(CoachVideoBody {
                    video_id: video_id_uuid::format(video.id()),
                    student_display_name: list
                        .students
                        .get(&video.student_id())
                        .map(|name| name.as_str().to_string()),
                    submitted_at: date_time_to_contract(video.submitted_at())?,
                    review_status: video_review_status_to_contract(video.status()),
                    returned_at: video
                        .review()
                        .map(|review| date_time_to_contract(review.returned_at()))
                        .transpose()?,
                    ai_feedback: ai_feedback_body(video)?,
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}
