//! `/message-threads/unanswered`。admin-web の `GET /message-threads/unanswered`（`listUnansweredMessageThreads`。
//! 本部が見る、返信のないまま時間の過ぎたスレッドの一覧）。本部だけが呼べる。クエリの `older_than_hours` がなければ
//! domain の [`UnansweredThreadCriteria`] の既定の時間（整数でない・範囲の外なら 422）。本部は件数と時間だけを見るので、
//! メッセージの本文・種類・スタンプは返さない。同じ接頭辞のスレッドの一覧・メッセージ・送信・既読は
//! [`crate::messaging`] で、スレッドの当事者だけ。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    messaging::{
        ListUnansweredMessageThreads, MessageThreadRepository, ThreadCoachDisplayNameLookup,
        ThreadStudentDisplayNameLookup, UnansweredThreadList, message_thread_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::{
    identity::Role,
    messaging::{MessageThread, MessagingError, UnansweredThreadCriteria},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{TokenVerification, require_role},
    contract::date_time_to_contract,
    error::ApiError,
};

/// `/message-threads/unanswered` のハンドラーが使うユースケース。
pub struct UnansweredMessageThreadsState<R, D, S, C> {
    pub list_unanswered_message_threads: ListUnansweredMessageThreads<R, D, S, C>,
}

/// admin-web の `listUnansweredMessageThreads` のクエリ。
#[derive(Deserialize)]
pub struct ListUnansweredMessageThreadsParams {
    older_than_hours: Option<String>,
}

/// admin-web の `UnansweredMessageThreadList`。
#[derive(Serialize)]
pub struct UnansweredMessageThreadListBody {
    older_than_hours: u16,
    threads: Vec<UnansweredMessageThreadBody>,
}

/// admin-web の `UnansweredMessageThread`。表示名が引けなければ項目ごと載せない。本文は載せない。
#[derive(Serialize)]
pub struct UnansweredMessageThreadBody {
    thread_id: String,
    coach_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    student_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
    last_student_message_at: String,
    elapsed_sec: u64,
}

/// `GET /message-threads/unanswered`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。
/// `older_than_hours` が整数でなければ 422、範囲の外も 422（範囲は domain の [`UnansweredThreadCriteria::new`]）。
pub async fn list_unanswered_message_threads<R, D, S, C>(
    State(state): State<Arc<UnansweredMessageThreadsState<R, D, S, C>>>,
    params: Result<Query<ListUnansweredMessageThreadsParams>, QueryRejection>,
) -> Result<Json<UnansweredMessageThreadListBody>, ApiError>
where
    R: MessageThreadRepository,
    D: ThreadCoachDisplayNameLookup,
    S: ThreadStudentDisplayNameLookup,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let criteria = match params.older_than_hours.as_deref() {
        None => UnansweredThreadCriteria::default(),
        Some(value) => {
            let hours = value
                .parse::<i64>()
                .map_err(|_| ApiError::validation_failed("older_than_hours が整数でない"))?;
            let hours = u16::try_from(hours).map_err(|_| {
                MessagingError::UnansweredThreadHoursOutOfRange {
                    min: UnansweredThreadCriteria::MIN_HOURS,
                    max: UnansweredThreadCriteria::MAX_HOURS,
                }
            })?;
            UnansweredThreadCriteria::new(hours)?
        }
    };
    let list = state
        .list_unanswered_message_threads
        .execute(criteria)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(UnansweredMessageThreadListBody {
        older_than_hours: list.criteria.hours(),
        threads: list
            .threads
            .iter()
            .map(|thread| unanswered_thread_body(thread, &list))
            .collect::<Result<_, _>>()?,
    }))
}

/// `list` のスレッド `thread` 1 件。表示名は `list` にあれば載せる。経過時間は照会した時刻まで。
fn unanswered_thread_body(
    thread: &MessageThread,
    list: &UnansweredThreadList,
) -> Result<UnansweredMessageThreadBody, ApiError> {
    // 一覧のスレッドは domain の基準で選んだものなので、生徒の最後の発言は必ずある
    let since = thread
        .awaiting_coach_reply_since()
        .ok_or_else(|| ApiError::internal("返信を待っていないスレッドが一覧にある"))?;
    let elapsed = UnansweredThreadCriteria::elapsed(thread, list.listed_at).unwrap_or_default();
    Ok(UnansweredMessageThreadBody {
        thread_id: message_thread_id_uuid::format(thread.id()),
        coach_id: account_id_uuid::format(thread.coach_id()),
        coach_display_name: list
            .coach_display_names
            .get(&thread.coach_id())
            .map(|name| name.as_str().to_string()),
        student_id: account_id_uuid::format(thread.student_id()),
        student_display_name: list
            .student_display_names
            .get(&thread.student_id())
            .map(|name| name.as_str().to_string()),
        last_student_message_at: date_time_to_contract(since)?,
        elapsed_sec: elapsed.as_secs(),
    })
}

/// 本部が見る、返信のないまま時間の過ぎたスレッドの一覧（admin-web の `GET /message-threads/unanswered`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /message-threads/unanswered` は本部だけ（コーチ・生徒は 403）。同じ接頭辞のスレッドの操作は
/// [`crate::messaging_router`] で、スレッドの当事者だけ。
pub fn unanswered_message_threads_router<R, D, S, C, T>(
    list_unanswered_message_threads: ListUnansweredMessageThreads<R, D, S, C>,
    tokens: T,
) -> Router
where
    R: MessageThreadRepository + 'static,
    D: ThreadCoachDisplayNameLookup + 'static,
    S: ThreadStudentDisplayNameLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/message-threads/unanswered",
            get(self::list_unanswered_message_threads::<R, D, S, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(UnansweredMessageThreadsState {
            list_unanswered_message_threads,
        }))
        .layer(TokenVerification::layer(tokens))
}
