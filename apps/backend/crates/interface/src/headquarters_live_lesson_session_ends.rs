//! 本部によるライブレッスンの終了。admin-web の `POST /live-lesson-sessions/{session_id}/headquarters-end`
//! （`endLiveLessonSessionByHeadquarters`。本部だけ。D188）。終了の記録がないまま残った通話セッション
//! （[`crate::overdue_live_lesson_sessions`] の一覧で見つけたもの）を、本部の判断で終わったことにする。
//! 参加者が通話を終える `POST /live-lesson-sessions/{session_id}/end` は [`crate::live_lesson_sessions`]。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    live_lesson::{
        EndLiveLessonSessionByHeadquarters, EndLiveLessonSessionByHeadquartersCommand,
        EndLiveLessonSessionByHeadquartersError, LiveLessonSessionRepository, session_id_uuid,
    },
    scheduling::booking_id_uuid,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use domain::{DomainError, identity::Role, live_lesson::LiveLessonSession};
use serde::Serialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::date_time_to_contract,
    error::ApiError,
    message,
};

/// `/live-lesson-sessions/{session_id}/headquarters-end` のハンドラーが使うユースケース。
pub struct HeadquartersLiveLessonSessionEndsState<S, C> {
    pub end_live_lesson_session_by_headquarters: EndLiveLessonSessionByHeadquarters<S, C>,
}

/// ハンドラーが共有する [`HeadquartersLiveLessonSessionEndsState`]。
type Shared<S, C> = Arc<HeadquartersLiveLessonSessionEndsState<S, C>>;

/// admin-web の `HeadquartersEndedLiveLessonSession`。
#[derive(Serialize)]
pub struct HeadquartersEndedLiveLessonSessionBody {
    session_id: String,
    booking_id: String,
    coach_id: String,
    student_id: String,
    started_at: String,
    ended_at: String,
    ended_by: String,
}

/// `POST /live-lesson-sessions/{session_id}/headquarters-end`（admin-web）。本部だけが呼べ（役割は `require_role` の層が
/// 確かめる）、終わらせたのはトークンのアカウント。リクエストの本文は読まない。UUID でない・ない ID は 404、終了済みは
/// 409 `invalid_session_state`（参加者の `endLiveLessonSession` と同じ code）。
pub async fn end_live_lesson_session_by_headquarters<S, C>(
    State(state): State<Shared<S, C>>,
    account: AuthenticatedAccount,
    Path(session_id): Path<String>,
) -> Result<Json<HeadquartersEndedLiveLessonSessionBody>, ApiError>
where
    S: LiveLessonSessionRepository,
    C: Clock,
{
    // 契約に 422 はないので、UUID でない ID はないセッションとして扱う
    let session_id = session_id_uuid::parse(&session_id).map_err(|_| session_not_found())?;
    let session = state
        .end_live_lesson_session_by_headquarters
        .execute(EndLiveLessonSessionByHeadquartersCommand {
            account_id: account.account_id(),
            session_id,
        })
        .await
        .map_err(|error| match error {
            EndLiveLessonSessionByHeadquartersError::SessionNotFound => session_not_found(),
            EndLiveLessonSessionByHeadquartersError::Invalid(
                error @ DomainError::LiveLessonSessionAlreadyEnded,
            ) => ApiError::conflict("invalid_session_state", message::domain_error(&error)),
            EndLiveLessonSessionByHeadquartersError::Invalid(error) => ApiError::internal(error),
            EndLiveLessonSessionByHeadquartersError::Unavailable(error) => {
                ApiError::internal(error)
            }
        })?;
    Ok(Json(ended_session_body(&session)?))
}

fn session_not_found() -> ApiError {
    ApiError::not_found("通話セッションがない")
}

fn ended_session_body(
    session: &LiveLessonSession,
) -> Result<HeadquartersEndedLiveLessonSessionBody, ApiError> {
    let ended_at = session
        .ended_at()
        .ok_or_else(|| ApiError::internal("本部が終わらせたセッションに終了時刻がない"))?;
    let ended_by = session
        .ended_by_headquarters()
        .ok_or_else(|| ApiError::internal("本部が終わらせたセッションに本部のアカウントがない"))?;
    Ok(HeadquartersEndedLiveLessonSessionBody {
        session_id: session_id_uuid::format(session.id()),
        booking_id: booking_id_uuid::format(session.booking_id()),
        coach_id: account_id_uuid::format(session.coach_id()),
        student_id: account_id_uuid::format(session.student_id()),
        started_at: date_time_to_contract(session.started_at())?,
        ended_at: date_time_to_contract(ended_at)?,
        ended_by: account_id_uuid::format(ended_by),
    })
}

/// 本部によるライブレッスンの終了（admin-web の `POST /live-lesson-sessions/{session_id}/headquarters-end`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// 本部だけ（コーチ・生徒は 403）。
pub fn headquarters_live_lesson_session_ends_router<S, C, T>(
    end_live_lesson_session_by_headquarters: EndLiveLessonSessionByHeadquarters<S, C>,
    tokens: T,
) -> Router
where
    S: LiveLessonSessionRepository + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/live-lesson-sessions/{session_id}/headquarters-end",
            post(self::end_live_lesson_session_by_headquarters::<S, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(HeadquartersLiveLessonSessionEndsState {
            end_live_lesson_session_by_headquarters,
        }))
        .layer(TokenVerification::layer(tokens))
}
