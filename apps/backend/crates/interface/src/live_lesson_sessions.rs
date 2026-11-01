//! `/live-lesson-sessions`。student-app の `POST /live-lesson-sessions`（`startLiveLessonSession`）と
//! `POST /live-lesson-sessions/{session_id}/end`（`endLiveLessonSession`）。予約の参加者（生徒・コーチ）だけが
//! 扱え、それ以外のアカウントには予約・セッションがあることも知らせない（404）。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    live_lesson::{
        BookingLookup, EndLiveLessonSession, EndLiveLessonSessionCommand,
        EndLiveLessonSessionError, LiveLessonSessionRepository, StartLiveLessonSession,
        StartLiveLessonSessionCommand, StartLiveLessonSessionError, VideoCallProvider,
        session_id_uuid,
    },
    scheduling::booking_id_uuid,
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    routing::post,
};
use domain::{DomainError, live_lesson::LiveLessonSession};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification},
    contract::{date_time_to_contract, session_status_to_contract},
    error::ApiError,
    message,
};

/// `/live-lesson-sessions` のハンドラーが使うユースケース。
pub struct LiveLessonSessionsState<L, K, V, A, C> {
    pub start_live_lesson_session: StartLiveLessonSession<L, K, V, A, C>,
    pub end_live_lesson_session: EndLiveLessonSession<L, V, A, C>,
}

/// ハンドラーが共有する [`LiveLessonSessionsState`]。
type Shared<L, K, V, A, C> = Arc<LiveLessonSessionsState<L, K, V, A, C>>;

/// student-app の `StartLiveLessonSessionRequest`。
#[derive(Deserialize)]
pub struct StartLiveLessonSessionRequest {
    booking_id: String,
}

/// student-app の `LiveLessonSession`。接続品質の指標と通話品質スコアは返さない（指標は行動イベント
/// `call.ended` に載せる）。`booking_id` / `student_id` は開始した予約、`bookings` は結び付いた予約の一覧
/// （グループレッスンでは人数分。先頭は開始した予約）。
#[derive(Serialize)]
pub struct LiveLessonSessionBody {
    session_id: String,
    booking_id: String,
    student_id: String,
    coach_id: String,
    status: &'static str,
    started_at: String,
    ended_at: Option<String>,
    duration_sec: Option<u64>,
    room_token: Option<String>,
    bookings: Vec<LiveLessonSessionBookingBody>,
}

/// student-app の `LiveLessonSessionBooking`（通話セッションに結び付いた予約）。
#[derive(Serialize)]
pub struct LiveLessonSessionBookingBody {
    booking_id: String,
    student_id: String,
}

/// `POST /live-lesson-sessions`。確定した予約の参加者（生徒・コーチ）が通話を始める。グループレッスンの通話中の
/// セッションに結び付いた予約なら、同じセッションに入る（201 で同じセッションを返す）。
pub async fn start_live_lesson_session<L, K, V, A, C>(
    State(state): State<Shared<L, K, V, A, C>>,
    account: AuthenticatedAccount,
    request: Result<Json<StartLiveLessonSessionRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<LiveLessonSessionBody>), ApiError>
where
    L: LiveLessonSessionRepository,
    K: BookingLookup,
    V: VideoCallProvider,
    A: AnalyticsSink,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let command = StartLiveLessonSessionCommand {
        account_id: account.account_id(),
        booking_id: booking_id_uuid::parse(&request.booking_id)
            .map_err(|_| ApiError::validation_failed("booking_id が UUID でない"))?,
    };

    let started = state
        .start_live_lesson_session
        .execute(command)
        .await
        .map_err(|error| match error {
            // 参加者でない予約は、あることも知らせない
            StartLiveLessonSessionError::BookingNotFound
            | StartLiveLessonSessionError::Invalid(DomainError::NotASessionParticipant) => {
                ApiError::not_found("予約がない")
            }
            StartLiveLessonSessionError::Invalid(
                error @ DomainError::BookingNotConfirmed { .. },
            ) => ApiError::conflict("invalid_booking_state", message::domain_error(&error)),
            StartLiveLessonSessionError::Invalid(
                error @ DomainError::LiveLessonSessionAlreadyStarted,
            ) => ApiError::conflict("invalid_session_state", message::domain_error(&error)),
            StartLiveLessonSessionError::Invalid(error) => ApiError::internal(error),
            StartLiveLessonSessionError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok((
        StatusCode::CREATED,
        Json(session_body(&started.session, Some(started.room_token))?),
    ))
}

/// `POST /live-lesson-sessions/{session_id}/end`。参加者が通話を終える。接続品質の指標は通話基盤（モック）が
/// サーバー側で生成し、行動イベント `call.ended` に載せる（レスポンスには含めない）。
pub async fn end_live_lesson_session<L, K, V, A, C>(
    State(state): State<Shared<L, K, V, A, C>>,
    account: AuthenticatedAccount,
    Path(session_id): Path<String>,
) -> Result<Json<LiveLessonSessionBody>, ApiError>
where
    L: LiveLessonSessionRepository,
    K: BookingLookup,
    V: VideoCallProvider,
    A: AnalyticsSink,
    C: Clock,
{
    // 契約の endLiveLessonSession に 422 はないので、UUID でない ID はないセッションとして扱う
    let session_id = session_id_uuid::parse(&session_id).map_err(|_| session_not_found())?;

    let session = state
        .end_live_lesson_session
        .execute(EndLiveLessonSessionCommand {
            account_id: account.account_id(),
            session_id,
        })
        .await
        .map_err(|error| match error {
            // 参加者でないセッションは、あることも知らせない
            EndLiveLessonSessionError::SessionNotFound
            | EndLiveLessonSessionError::Invalid(DomainError::NotASessionParticipant) => {
                session_not_found()
            }
            EndLiveLessonSessionError::Invalid(
                error @ DomainError::LiveLessonSessionAlreadyEnded,
            ) => ApiError::conflict("invalid_session_state", message::domain_error(&error)),
            EndLiveLessonSessionError::Invalid(error) => ApiError::internal(error),
            EndLiveLessonSessionError::Unavailable(error) => ApiError::internal(error),
        })?;
    // 終了したルームには参加できないので、トークンは返さない
    Ok(Json(session_body(&session, None)?))
}

fn session_not_found() -> ApiError {
    ApiError::not_found("通話セッションがない")
}

fn session_body(
    session: &LiveLessonSession,
    room_token: Option<String>,
) -> Result<LiveLessonSessionBody, ApiError> {
    Ok(LiveLessonSessionBody {
        session_id: session_id_uuid::format(session.id()),
        booking_id: booking_id_uuid::format(session.booking_id()),
        student_id: account_id_uuid::format(session.student_id()),
        coach_id: account_id_uuid::format(session.coach_id()),
        status: session_status_to_contract(session.status()),
        started_at: date_time_to_contract(session.started_at())?,
        ended_at: session.ended_at().map(date_time_to_contract).transpose()?,
        duration_sec: session.duration().map(|duration| duration.as_secs()),
        room_token,
        bookings: session
            .bookings()
            .iter()
            .map(|booking| LiveLessonSessionBookingBody {
                booking_id: booking_id_uuid::format(booking.booking_id()),
                student_id: account_id_uuid::format(booking.student_id()),
            })
            .collect(),
    })
}

/// 通話セッションの開始・終了（`POST /live-lesson-sessions`・`POST /live-lesson-sessions/{session_id}/end`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// 役割で絞らず、予約の参加者（生徒・コーチ）かをユースケースが確かめる（契約に 403 がない）。
pub fn live_lesson_sessions_router<L, K, V, A, C, T>(
    start_live_lesson_session: StartLiveLessonSession<L, K, V, A, C>,
    end_live_lesson_session: EndLiveLessonSession<L, V, A, C>,
    tokens: T,
) -> Router
where
    L: LiveLessonSessionRepository + 'static,
    K: BookingLookup + 'static,
    V: VideoCallProvider + 'static,
    A: AnalyticsSink + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/live-lesson-sessions",
            post(self::start_live_lesson_session::<L, K, V, A, C>),
        )
        .route(
            "/live-lesson-sessions/{session_id}/end",
            post(self::end_live_lesson_session::<L, K, V, A, C>),
        )
        .with_state(Arc::new(LiveLessonSessionsState {
            start_live_lesson_session,
            end_live_lesson_session,
        }))
        .layer(TokenVerification::layer(tokens))
}
