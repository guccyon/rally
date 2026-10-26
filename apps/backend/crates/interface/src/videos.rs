//! `/videos`。student-app の `POST /videos`（`submitVideo`。生徒だけ）と `GET /videos/{video_id}/review`
//! （`getVideoReview`。動画の当事者だけ）、admin-web の `POST /videos/{video_id}/review`（`returnVideoReview`。
//! コーチだけ）。当事者でないアカウントには動画があることも知らせない（404）。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    async_lesson::{
        GetVideoReview, GetVideoReviewError, GetVideoReviewQuery, ReturnVideoReview,
        ReturnVideoReviewCommand, ReturnVideoReviewError, ReviewGate, SubmitVideo,
        SubmitVideoCommand, SubmitVideoError, VideoRepository, VideoStorage, video_id_uuid,
        video_review_id_uuid,
    },
    clock::Clock,
    identity::account_id_uuid,
};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
};
use domain::{
    DomainError,
    async_lesson::{ReviewComment, Video},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedAccount,
    contract::{
        VIDEO_STATUS_AWAITING_UPLOAD, date_time_to_contract, sport_to_contract, student_app_sport,
        video_content_type_from_contract, video_review_status_to_contract,
    },
    error::ApiError,
    message,
};

/// student-app の `SubmitVideoRequest.note` の `maxLength`（文字数）。
const NOTE_MAX_CHARS: usize = 1000;

/// `/videos` のハンドラーが使うユースケース。
pub struct VideosState<V, S, G, A, C> {
    pub submit_video: SubmitVideo<V, S, A, C>,
    pub get_video_review: GetVideoReview<V>,
    pub return_video_review: ReturnVideoReview<V, G, A, C>,
}

/// ハンドラーが共有する [`VideosState`]。
type Shared<V, S, G, A, C> = Arc<VideosState<V, S, G, A, C>>;

/// student-app の `SubmitVideoRequest`。
#[derive(Deserialize)]
pub struct SubmitVideoRequest {
    coach_id: String,
    sport: String,
    content_type: String,
    note: Option<String>,
}

/// student-app の `Video`。
#[derive(Serialize)]
pub struct VideoBody {
    video_id: String,
    student_id: String,
    coach_id: String,
    sport: &'static str,
    status: &'static str,
    upload_url: Option<String>,
    created_at: String,
}

/// admin-web の `ReturnVideoReviewRequest`。
#[derive(Deserialize)]
pub struct ReturnVideoReviewRequest {
    comment: String,
}

/// student-app・admin-web の `VideoReview`。
#[derive(Serialize)]
pub struct VideoReviewBody {
    review_id: Option<String>,
    video_id: String,
    student_id: String,
    coach_id: String,
    status: &'static str,
    comment: Option<String>,
    returned_at: Option<String>,
}

/// `POST /videos`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。送る生徒はトークンのアカウント。
///
/// `note`（コーチへの相談内容）は長さだけを確かめ、Gen1 では保存しない。
pub async fn submit_video<V, S, G, A, C>(
    State(state): State<Shared<V, S, G, A, C>>,
    account: AuthenticatedAccount,
    request: Result<Json<SubmitVideoRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<VideoBody>), ApiError>
where
    V: VideoRepository,
    S: VideoStorage,
    G: ReviewGate,
    A: AnalyticsSink,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    if request
        .note
        .as_ref()
        .is_some_and(|note| note.chars().count() > NOTE_MAX_CHARS)
    {
        return Err(ApiError::validation_failed(format!(
            "note は {NOTE_MAX_CHARS} 文字まで"
        )));
    }
    let command = SubmitVideoCommand {
        student_id: account.account_id(),
        coach_id: account_id_uuid::parse(&request.coach_id)
            .map_err(|_| ApiError::validation_failed("coach_id が UUID でない"))?,
        sport: student_app_sport(&request.sport)?,
        content_type: video_content_type_from_contract(&request.content_type)?,
    };

    let submitted = state
        .submit_video
        .execute(command)
        .await
        .map_err(|error| match error {
            SubmitVideoError::Unavailable(error) => ApiError::internal(error),
        })?;
    let video = &submitted.video;
    Ok((
        StatusCode::CREATED,
        Json(VideoBody {
            video_id: video_id_uuid::format(video.id()),
            student_id: account_id_uuid::format(video.student_id()),
            coach_id: account_id_uuid::format(video.coach_id()),
            sport: sport_to_contract(video.sport()),
            status: VIDEO_STATUS_AWAITING_UPLOAD,
            upload_url: Some(submitted.upload_url),
            created_at: date_time_to_contract(video.submitted_at())?,
        }),
    ))
}

/// `GET /videos/{video_id}/review`。動画の当事者（送った生徒・送り先のコーチ）が添削を読む。コーチがまだ返して
/// いなければ `in_progress`（未返却）。
pub async fn get_video_review<V, S, G, A, C>(
    State(state): State<Shared<V, S, G, A, C>>,
    account: AuthenticatedAccount,
    Path(video_id): Path<String>,
) -> Result<Json<VideoReviewBody>, ApiError>
where
    V: VideoRepository,
    S: VideoStorage,
    G: ReviewGate,
    A: AnalyticsSink,
    C: Clock,
{
    // 契約の getVideoReview に 422 はないので、UUID でない ID はない動画として扱う
    let video_id = video_id_uuid::parse(&video_id).map_err(|_| video_not_found())?;

    let video = state
        .get_video_review
        .execute(GetVideoReviewQuery {
            account_id: account.account_id(),
            video_id,
        })
        .await
        .map_err(|error| match error {
            // 当事者でない動画は、あることも知らせない
            GetVideoReviewError::VideoNotFound
            | GetVideoReviewError::Invalid(DomainError::NotAVideoParty) => video_not_found(),
            GetVideoReviewError::Invalid(error) => ApiError::internal(error),
            GetVideoReviewError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(review_body(&video)?))
}

/// `POST /videos/{video_id}/review`（admin-web）。コーチだけが呼べ（役割は `require_role` の層が確かめる）、
/// 返せるのは自分に送られた動画だけ。
pub async fn return_video_review<V, S, G, A, C>(
    State(state): State<Shared<V, S, G, A, C>>,
    account: AuthenticatedAccount,
    Path(video_id): Path<String>,
    request: Result<Json<ReturnVideoReviewRequest>, JsonRejection>,
) -> Result<Json<VideoReviewBody>, ApiError>
where
    V: VideoRepository,
    S: VideoStorage,
    G: ReviewGate,
    A: AnalyticsSink,
    C: Clock,
{
    let video_id = video_id_uuid::parse(&video_id).map_err(|_| video_not_found())?;
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let command = ReturnVideoReviewCommand {
        coach_id: account.account_id(),
        video_id,
        comment: ReviewComment::new(request.comment)?,
    };

    let video = state
        .return_video_review
        .execute(command)
        .await
        .map_err(|error| match error {
            // 自分に送られていない動画は、あることも知らせない
            ReturnVideoReviewError::VideoNotFound
            | ReturnVideoReviewError::Invalid(DomainError::NotTheVideoCoach) => video_not_found(),
            ReturnVideoReviewError::Invalid(error @ DomainError::VideoReviewAlreadyReturned) => {
                ApiError::conflict("invalid_video_state", message::domain_error(&error))
            }
            ReturnVideoReviewError::Invalid(error @ DomainError::VideoReviewNotAllowed { .. }) => {
                ApiError::conflict(
                    "review_payment_not_confirmed",
                    message::domain_error(&error),
                )
            }
            ReturnVideoReviewError::Invalid(error) => ApiError::internal(error),
            ReturnVideoReviewError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(review_body(&video)?))
}

fn video_not_found() -> ApiError {
    ApiError::not_found("動画がない")
}

fn review_body(video: &Video) -> Result<VideoReviewBody, ApiError> {
    let review = video.review();
    Ok(VideoReviewBody {
        review_id: review.map(|review| video_review_id_uuid::format(review.id())),
        video_id: video_id_uuid::format(video.id()),
        student_id: account_id_uuid::format(video.student_id()),
        coach_id: account_id_uuid::format(video.coach_id()),
        status: video_review_status_to_contract(video.status()),
        comment: review.map(|review| review.comment().as_str().to_string()),
        returned_at: review
            .map(|review| date_time_to_contract(review.returned_at()))
            .transpose()?,
    })
}
