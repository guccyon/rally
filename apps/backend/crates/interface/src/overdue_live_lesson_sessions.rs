//! `/live-lesson-sessions/overdue`。admin-web の `GET /live-lesson-sessions/overdue`（`listOverdueLiveLessonSessions`。
//! 本部が見る、終わっていないまま時間の過ぎた通話セッションの一覧）。本部だけが呼べる。クエリの `older_than_hours` が
//! なければ domain の [`OverdueSessionCriteria`] の既定の時間（整数でない・範囲の外なら 422）。任意の `coach_id` で
//! そのコーチのセッションだけに絞れる（UUID でなければ 422）。任意の `sport` でその競技のコーチのセッションだけに絞れる
//! （`Sport` の値でなければ 422。競技の決め方は domain の [`OverdueSessionCriteria::matches_coach_sport`]）。任意の
//! `student_id` でその生徒の予約が結び付いたセッションだけに絞れる（UUID でなければ 422。`listLiveLessonSessions` の
//! `student_id` と同じ）。同じ接頭辞の
//! `POST /live-lesson-sessions`（開始）・`POST /live-lesson-sessions/{session_id}/end`（終了）は
//! [`crate::live_lesson_sessions`] で、予約の参加者だけ。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    live_lesson::{
        ListOverdueLiveLessonSessions, LiveLessonSessionRepository, OverdueSessionList,
        SessionCoachAccountLookup, SessionCoachDisplayNameLookup, SessionStudentDisplayNameLookup,
        session_id_uuid,
    },
    scheduling::booking_id_uuid,
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::{
    DomainError,
    identity::Role,
    live_lesson::{LiveLessonSession, OverdueSessionCriteria},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{TokenVerification, require_role},
    contract::{admin_web_sport, date_time_to_contract},
    error::ApiError,
};

/// `/live-lesson-sessions/overdue` のハンドラーが使うユースケース。
pub struct OverdueLiveLessonSessionsState<L, A, D, S, C> {
    pub list_overdue_live_lesson_sessions: ListOverdueLiveLessonSessions<L, A, D, S, C>,
}

/// ハンドラーが共有する [`OverdueLiveLessonSessionsState`]。
type Shared<L, A, D, S, C> = Arc<OverdueLiveLessonSessionsState<L, A, D, S, C>>;

/// admin-web の `listOverdueLiveLessonSessions` のクエリ。
#[derive(Deserialize)]
pub struct ListOverdueLiveLessonSessionsParams {
    older_than_hours: Option<String>,
    coach_id: Option<String>,
    sport: Option<String>,
    student_id: Option<String>,
}

/// admin-web の `OverdueLiveLessonSessionList`。
#[derive(Serialize)]
pub struct OverdueLiveLessonSessionListBody {
    older_than_hours: u16,
    sessions: Vec<OverdueLiveLessonSessionBody>,
}

/// admin-web の `OverdueLiveLessonSession`。表示名が引けなければ項目ごと載せない。
#[derive(Serialize)]
pub struct OverdueLiveLessonSessionBody {
    session_id: String,
    booking_id: String,
    coach_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    student_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
    started_at: String,
    elapsed_sec: u64,
}

/// `GET /live-lesson-sessions/overdue`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。
/// `older_than_hours` が整数でなければ 422、範囲の外も 422（範囲は domain の [`OverdueSessionCriteria::new`]）。
/// `coach_id` が UUID でなければ 422（コーチでない ID・ない ID は空の一覧）。`sport` が `Sport` の値でなければ 422
/// （`listPendingVideoReviews` の `sport` と同じ）。`student_id` が UUID でなければ 422（生徒でない ID・ない ID は空の
/// 一覧。`listLiveLessonSessions` の `student_id` と同じ）。
pub async fn list_overdue_live_lesson_sessions<L, A, D, S, C>(
    State(state): State<Shared<L, A, D, S, C>>,
    params: Result<Query<ListOverdueLiveLessonSessionsParams>, QueryRejection>,
) -> Result<Json<OverdueLiveLessonSessionListBody>, ApiError>
where
    L: LiveLessonSessionRepository,
    A: SessionCoachAccountLookup,
    D: SessionCoachDisplayNameLookup,
    S: SessionStudentDisplayNameLookup,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let criteria = match params.older_than_hours.as_deref() {
        None => OverdueSessionCriteria::default(),
        Some(value) => {
            let hours = value
                .parse::<i64>()
                .map_err(|_| ApiError::validation_failed("older_than_hours が整数でない"))?;
            let hours =
                u16::try_from(hours).map_err(|_| DomainError::OverdueSessionHoursOutOfRange {
                    min: OverdueSessionCriteria::MIN_HOURS,
                    max: OverdueSessionCriteria::MAX_HOURS,
                })?;
            OverdueSessionCriteria::new(hours)?
        }
    };
    let optional_account_id = |field: &str, value: Option<&str>| {
        value
            .map(|id| {
                account_id_uuid::parse(id)
                    .map_err(|_| ApiError::validation_failed(format!("{field} が UUID でない")))
            })
            .transpose()
    };
    let coach_id = optional_account_id("coach_id", params.coach_id.as_deref())?;
    let student_id = optional_account_id("student_id", params.student_id.as_deref())?;
    let sport = params.sport.as_deref().map(admin_web_sport).transpose()?;
    let criteria = criteria
        .with_coach_id(coach_id)
        .with_sport(sport)
        .with_student_id(student_id);
    let list = state
        .list_overdue_live_lesson_sessions
        .execute(criteria)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(OverdueLiveLessonSessionListBody {
        older_than_hours: list.criteria.hours(),
        sessions: list
            .sessions
            .iter()
            .map(|session| overdue_session_body(session, &list))
            .collect::<Result<_, _>>()?,
    }))
}

/// `list` のセッション `session` 1 件。表示名は `list` にあれば載せる。経過時間は照会した時刻まで。
fn overdue_session_body(
    session: &LiveLessonSession,
    list: &OverdueSessionList,
) -> Result<OverdueLiveLessonSessionBody, ApiError> {
    Ok(OverdueLiveLessonSessionBody {
        session_id: session_id_uuid::format(session.id()),
        booking_id: booking_id_uuid::format(session.booking_id()),
        coach_id: account_id_uuid::format(session.coach_id()),
        coach_display_name: list
            .coach_display_names
            .get(&session.coach_id())
            .map(|name| name.as_str().to_string()),
        student_id: account_id_uuid::format(session.student_id()),
        student_display_name: list
            .student_display_names
            .get(&session.student_id())
            .map(|name| name.as_str().to_string()),
        started_at: date_time_to_contract(session.started_at())?,
        elapsed_sec: session.elapsed(list.listed_at).as_secs(),
    })
}

/// 本部が見る、終わっていないまま時間の過ぎた通話セッションの一覧（admin-web の `GET /live-lesson-sessions/overdue`）の
/// ルーター。組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /live-lesson-sessions/overdue` は本部だけ（コーチ・生徒は 403）。同じ接頭辞の開始・終了は
/// [`crate::live_lesson_sessions_router`] で、予約の参加者だけ。
pub fn overdue_live_lesson_sessions_router<L, A, D, S, C, T>(
    list_overdue_live_lesson_sessions: ListOverdueLiveLessonSessions<L, A, D, S, C>,
    tokens: T,
) -> Router
where
    L: LiveLessonSessionRepository + 'static,
    A: SessionCoachAccountLookup + 'static,
    D: SessionCoachDisplayNameLookup + 'static,
    S: SessionStudentDisplayNameLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/live-lesson-sessions/overdue",
            get(self::list_overdue_live_lesson_sessions::<L, A, D, S, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(OverdueLiveLessonSessionsState {
            list_overdue_live_lesson_sessions,
        }))
        .layer(TokenVerification::layer(tokens))
}
