//! `GET /live-lesson-sessions`。admin-web の `listLiveLessonSessions`（本部が期間とコーチ・生徒で見る通話セッションの記録の
//! 一覧）。本部だけが呼べる。同じパスの `POST /live-lesson-sessions`（開始。予約の参加者だけ）は
//! [`crate::live_lesson_sessions`]、終わっていないまま時間の過ぎたセッションの一覧は
//! [`crate::overdue_live_lesson_sessions`]。

use std::sync::Arc;

use application::{
    identity::{TokenVerifier, account_id_uuid},
    live_lesson::{
        ListLiveLessonSessions, ListLiveLessonSessionsError, ListLiveLessonSessionsQuery,
        LiveLessonSessionHistory, LiveLessonSessionRepository, SessionCoachDisplayNameLookup,
        SessionStudentDisplayNameLookup, session_id_uuid,
    },
    scheduling::booking_id_uuid,
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::{identity::Role, live_lesson::LiveLessonSession};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{TokenVerification, require_role},
    contract::{date_from_contract, date_time_to_contract, date_to_contract},
    error::ApiError,
};

/// `GET /live-lesson-sessions` のハンドラーが使うユースケース。
pub struct HeadquartersLiveLessonSessionsState<L, D, S> {
    pub list_live_lesson_sessions: ListLiveLessonSessions<L, D, S>,
}

/// admin-web の `listLiveLessonSessions` のクエリ。`from`・`to` は契約では必須で、ないときはハンドラーが 422 にする。
#[derive(Deserialize)]
pub struct ListLiveLessonSessionsParams {
    from: Option<String>,
    to: Option<String>,
    coach_id: Option<String>,
    student_id: Option<String>,
}

/// admin-web の `LiveLessonSessionRecordList`。
#[derive(Serialize)]
pub struct LiveLessonSessionRecordListBody {
    from: String,
    to: String,
    sessions: Vec<LiveLessonSessionRecordBody>,
}

/// admin-web の `LiveLessonSessionRecord`。表示名が引けなければ項目ごと載せない。終了していなければ `ended_at`・
/// `duration_minutes` は null。
#[derive(Serialize)]
pub struct LiveLessonSessionRecordBody {
    session_id: String,
    booking_id: String,
    coach_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    student_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
    started_at: String,
    ended_at: Option<String>,
    duration_minutes: Option<u64>,
}

/// `GET /live-lesson-sessions`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。`from`・`to` が
/// ない・`YYYY-MM-DD` の日付でないとき、`coach_id`・`student_id` が UUID でないときと、期間の違反（domain の
/// [`domain::live_lesson::SessionHistoryCriteria::new`]）は 422。
pub async fn list_live_lesson_sessions<L, D, S>(
    State(state): State<Arc<HeadquartersLiveLessonSessionsState<L, D, S>>>,
    params: Result<Query<ListLiveLessonSessionsParams>, QueryRejection>,
) -> Result<Json<LiveLessonSessionRecordListBody>, ApiError>
where
    L: LiveLessonSessionRepository,
    D: SessionCoachDisplayNameLookup,
    S: SessionStudentDisplayNameLookup,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let required_date = |field: &str, value: Option<&str>| match value {
        Some(value) => date_from_contract(field, value),
        None => Err(ApiError::validation_failed(format!("{field} がない"))),
    };
    let optional_account_id = |field: &str, value: Option<&str>| {
        value
            .map(|id| {
                account_id_uuid::parse(id)
                    .map_err(|_| ApiError::validation_failed(format!("{field} が UUID でない")))
            })
            .transpose()
    };
    let query = ListLiveLessonSessionsQuery {
        from: required_date("from", params.from.as_deref())?,
        to: required_date("to", params.to.as_deref())?,
        coach_id: optional_account_id("coach_id", params.coach_id.as_deref())?,
        student_id: optional_account_id("student_id", params.student_id.as_deref())?,
    };
    let history = state
        .list_live_lesson_sessions
        .execute(query)
        .await
        .map_err(|error| match error {
            ListLiveLessonSessionsError::Invalid(error) => ApiError::from(error),
            ListLiveLessonSessionsError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(LiveLessonSessionRecordListBody {
        from: date_to_contract(history.from),
        to: date_to_contract(history.to),
        sessions: history
            .sessions
            .iter()
            .map(|session| session_record_body(session, &history))
            .collect::<Result<_, _>>()?,
    }))
}

/// `history` のセッション `session` 1 件。表示名は `history` にあれば載せる。長さは開始から終了までの分（切り捨て）。
fn session_record_body(
    session: &LiveLessonSession,
    history: &LiveLessonSessionHistory,
) -> Result<LiveLessonSessionRecordBody, ApiError> {
    Ok(LiveLessonSessionRecordBody {
        session_id: session_id_uuid::format(session.id()),
        booking_id: booking_id_uuid::format(session.booking_id()),
        coach_id: account_id_uuid::format(session.coach_id()),
        coach_display_name: history
            .coach_display_names
            .get(&session.coach_id())
            .map(|name| name.as_str().to_string()),
        student_id: account_id_uuid::format(session.student_id()),
        student_display_name: history
            .student_display_names
            .get(&session.student_id())
            .map(|name| name.as_str().to_string()),
        started_at: date_time_to_contract(session.started_at())?,
        ended_at: session.ended_at().map(date_time_to_contract).transpose()?,
        duration_minutes: session.duration().map(|duration| duration.as_secs() / 60),
    })
}

/// 本部が期間とコーチ・生徒で見る通話セッションの記録の一覧（admin-web の `GET /live-lesson-sessions`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。同じパスの `POST /live-lesson-sessions`
/// （開始。予約の参加者だけ）は [`crate::live_lesson_sessions_router`] にあり、`merge` で 1 つのパスにまとまる。
///
/// `GET /live-lesson-sessions` は本部だけ（コーチ・生徒は 403）。
pub fn headquarters_live_lesson_sessions_router<L, D, S, T>(
    list_live_lesson_sessions: ListLiveLessonSessions<L, D, S>,
    tokens: T,
) -> Router
where
    L: LiveLessonSessionRepository + 'static,
    D: SessionCoachDisplayNameLookup + 'static,
    S: SessionStudentDisplayNameLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/live-lesson-sessions",
            get(self::list_live_lesson_sessions::<L, D, S>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(HeadquartersLiveLessonSessionsState {
            list_live_lesson_sessions,
        }))
        .layer(TokenVerification::layer(tokens))
}
