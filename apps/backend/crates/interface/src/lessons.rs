//! `/lessons/completions`。student-app の `POST /lessons/completions`（`recordLessonCompletion`）。
//!
//! 同期レッスンは終了した通話セッション、非同期レッスンは添削が返された動画を指定する。記録できるのは元の
//! セッション・動画の当事者（生徒・コーチ）だけで、役割では絞らない（契約に 403 がない）。それ以外のアカウントには
//! セッション・動画があることも知らせない（404）。`is_first_lesson` はサーバーが判定する。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    async_lesson::{VideoRepository, video_id_uuid},
    clock::Clock,
    domain_event::DomainEventPublisher,
    identity::account_id_uuid,
    lesson::{
        LessonCompletionRepository, RecordLessonCompletion, RecordLessonCompletionCommand,
        RecordLessonCompletionError, lesson_completion_id_uuid,
    },
    live_lesson::{LiveLessonSessionRepository, session_id_uuid},
};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
};
use domain::{
    DomainError,
    lesson::{LessonCompletion, LessonSource, LessonType},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedAccount,
    contract::{date_time_to_contract, lesson_type_from_contract, lesson_type_to_contract},
    error::ApiError,
    message,
};

/// `/lessons/completions` のハンドラーが使うユースケース。
pub struct LessonsState<L, V, R, A, P, C> {
    pub record_lesson_completion: RecordLessonCompletion<L, V, R, A, P, C>,
}

/// ハンドラーが共有する [`LessonsState`]。
type Shared<L, V, R, A, P, C> = Arc<LessonsState<L, V, R, A, P, C>>;

/// student-app の `RecordLessonCompletionRequest`（`lesson_type` で判別する `LiveLessonCompletion` /
/// `AsyncLessonCompletion`）。どちらも `additionalProperties: false` なので、知らない項目は 422。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordLessonCompletionRequest {
    lesson_type: String,
    session_id: Option<String>,
    video_id: Option<String>,
}

/// student-app の `Lesson`。
#[derive(Serialize)]
pub struct LessonBody {
    lesson_id: String,
    student_id: String,
    coach_id: String,
    lesson_type: &'static str,
    session_id: Option<String>,
    video_id: Option<String>,
    is_first_lesson: bool,
    completed_at: String,
}

/// `POST /lessons/completions`。通話・動画添削の当事者がレッスンの完了を記録する。
pub async fn record_lesson_completion<L, V, R, A, P, C>(
    State(state): State<Shared<L, V, R, A, P, C>>,
    account: AuthenticatedAccount,
    request: Result<Json<RecordLessonCompletionRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<LessonBody>), ApiError>
where
    L: LiveLessonSessionRepository,
    V: VideoRepository,
    R: LessonCompletionRepository,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let command = RecordLessonCompletionCommand {
        account_id: account.account_id(),
        source: lesson_source(request)?,
    };

    let completion = state
        .record_lesson_completion
        .execute(command)
        .await
        .map_err(|error| match error {
            // 当事者でないセッション・動画は、あることも知らせない
            RecordLessonCompletionError::SourceNotFound
            | RecordLessonCompletionError::Invalid(
                DomainError::NotASessionParticipant | DomainError::NotAVideoParty,
            ) => ApiError::not_found("通話セッション・動画がない"),
            RecordLessonCompletionError::Invalid(
                error @ DomainError::LiveLessonSessionNotEnded,
            ) => ApiError::conflict("invalid_session_state", message::domain_error(&error)),
            RecordLessonCompletionError::Invalid(error @ DomainError::VideoReviewNotReturned) => {
                ApiError::conflict("invalid_video_state", message::domain_error(&error))
            }
            RecordLessonCompletionError::Invalid(error @ DomainError::LessonAlreadyCompleted) => {
                ApiError::conflict("lesson_already_completed", message::domain_error(&error))
            }
            RecordLessonCompletionError::Invalid(error) => ApiError::internal(error),
            RecordLessonCompletionError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok((StatusCode::CREATED, Json(lesson_body(&completion)?)))
}

/// 判別子 `lesson_type` に合う ID（同期は `session_id`、非同期は `video_id`）だけがあるかを確かめて読む。
fn lesson_source(request: RecordLessonCompletionRequest) -> Result<LessonSource, ApiError> {
    match (
        lesson_type_from_contract(&request.lesson_type)?,
        request.session_id,
        request.video_id,
    ) {
        (LessonType::Live, Some(session_id), None) => session_id_uuid::parse(&session_id)
            .map(LessonSource::Live)
            .map_err(|_| ApiError::validation_failed("session_id が UUID でない")),
        (LessonType::Async, None, Some(video_id)) => video_id_uuid::parse(&video_id)
            .map(LessonSource::Async)
            .map_err(|_| ApiError::validation_failed("video_id が UUID でない")),
        (LessonType::Live, ..) => Err(ApiError::validation_failed(
            "live_lesson は session_id だけを指定する",
        )),
        (LessonType::Async, ..) => Err(ApiError::validation_failed(
            "async_lesson は video_id だけを指定する",
        )),
    }
}

/// 完了したレッスンの元を、契約の `session_id`・`video_id`（種類に合わないほうは null）の組に写す。
pub(crate) fn source_ids(source: LessonSource) -> (Option<String>, Option<String>) {
    match source {
        LessonSource::Live(session_id) => (Some(session_id_uuid::format(session_id)), None),
        LessonSource::Async(video_id) => (None, Some(video_id_uuid::format(video_id))),
    }
}

fn lesson_body(completion: &LessonCompletion) -> Result<LessonBody, ApiError> {
    let (session_id, video_id) = source_ids(completion.source());
    Ok(LessonBody {
        lesson_id: lesson_completion_id_uuid::format(completion.id()),
        student_id: account_id_uuid::format(completion.student_id()),
        coach_id: account_id_uuid::format(completion.coach_id()),
        lesson_type: lesson_type_to_contract(completion.lesson_type()),
        session_id,
        video_id,
        is_first_lesson: completion.is_first_lesson(),
        completed_at: date_time_to_contract(completion.completed_at())?,
    })
}
