//! `/students`。admin-web の `GET /students`（`listStudents`。本部が見る生徒の一覧）。本部だけが呼べる。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{
        AccountRepository, ListStudents, ListStudentsQuery, StudentCoachBookingLookup,
        StudentContractLookup, StudentLastBookingLookup, StudentLowRatedReviewCountsLookup,
        StudentOverview, StudentPendingVideoReviewCountsLookup, StudentRecentBookingCountLookup,
        StudentUnansweredMessageThreadCountsLookup, TokenVerifier, account_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::identity::Role;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{TokenVerification, require_role},
    contract::{
        admin_web_sport, contract_type_to_contract, date_from_contract, date_time_to_contract,
        display_name_query_from_contract, sport_to_contract, subscription_plan_to_contract,
    },
    error::ApiError,
};

/// `/students` のハンドラーが使うユースケース。
pub struct StudentsState<R, C, B, K, N, U, P, W, L> {
    pub list_students: ListStudents<R, C, B, K, N, U, P, W, L>,
}

/// admin-web の `listStudents` のクエリ。
#[derive(Deserialize)]
pub struct ListStudentsParams {
    q: Option<String>,
    sport: Option<String>,
    not_booked_since: Option<String>,
    coach_id: Option<String>,
    has_unanswered: Option<bool>,
    has_pending_video: Option<bool>,
    has_unhandled_low_rated: Option<bool>,
}

/// admin-web の `StudentList`。
#[derive(Serialize)]
pub struct StudentListBody {
    students: Vec<StudentBody>,
}

/// admin-web の `Student`。メールアドレスは載せない。
#[derive(Serialize)]
pub struct StudentBody {
    student_id: String,
    display_name: String,
    sport: &'static str,
    contract_type: Option<&'static str>,
    plan: Option<&'static str>,
    last_booked_at: Option<String>,
    recent_booking_count: u32,
    registered_at: String,
}

fn student_body(student: &StudentOverview) -> Result<StudentBody, ApiError> {
    Ok(StudentBody {
        student_id: account_id_uuid::format(student.student_id),
        display_name: student.display_name.as_str().to_string(),
        sport: sport_to_contract(student.sport),
        contract_type: student
            .contract
            .map(|contract| contract_type_to_contract(contract.contract_type)),
        plan: student
            .contract
            .and_then(|contract| contract.plan)
            .map(subscription_plan_to_contract),
        last_booked_at: student
            .last_booked_at
            .map(date_time_to_contract)
            .transpose()?,
        recent_booking_count: student.recent_booking_count,
        registered_at: date_time_to_contract(student.registered_at)?,
    })
}

/// `GET /students`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。生徒を登録の新しい順で
/// 返す（ゴルフの生徒も載せる）。`q` があれば表示名に部分一致する生徒だけで、長すぎる `q` は 422。`sport` があれば
/// そのスポーツの生徒だけ、`not_booked_since`（`YYYY-MM-DD`。日本時間）があればその日以降に予約していない生徒だけで、
/// `coach_id` があればそのコーチの枠を一度でも予約した生徒だけ（キャンセル済みの予約も数える）で、ない・コーチでない ID なら
/// 空の一覧。`has_unanswered` が `true` なら返信のないまま既定の時間の過ぎたスレッドのある生徒だけ、`has_pending_video` が
/// `true` なら返却待ちの動画のある生徒だけ、`has_unhandled_low_rated` が `true` なら本部が未対応の直近の低い評価をつけた
/// 生徒だけ（どれも付けない・`false` なら絞らない）。`sport` が `Sport` の値でない・`not_booked_since` が日付でない・
/// `coach_id` が UUID でない・`has_unanswered`・`has_pending_video`・`has_unhandled_low_rated` が `true` / `false` でない
/// ときは 422。
/// 各行には直近 30 日の予約の数（`recent_booking_count`）を添える。
#[allow(clippy::type_complexity)]
pub async fn list_students<R, C, B, K, N, U, P, W, L>(
    State(state): State<Arc<StudentsState<R, C, B, K, N, U, P, W, L>>>,
    params: Result<Query<ListStudentsParams>, QueryRejection>,
) -> Result<Json<StudentListBody>, ApiError>
where
    R: AccountRepository,
    C: StudentContractLookup,
    B: StudentLastBookingLookup,
    K: StudentCoachBookingLookup,
    N: StudentRecentBookingCountLookup,
    U: StudentUnansweredMessageThreadCountsLookup,
    P: StudentPendingVideoReviewCountsLookup,
    W: StudentLowRatedReviewCountsLookup,
    L: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let query = ListStudentsQuery {
        name: display_name_query_from_contract(params.q.as_deref())?,
        sport: params.sport.as_deref().map(admin_web_sport).transpose()?,
        not_booked_since: params
            .not_booked_since
            .as_deref()
            .map(|value| date_from_contract("not_booked_since", value))
            .transpose()?,
        coach_id: params
            .coach_id
            .as_deref()
            .map(|id| {
                account_id_uuid::parse(id)
                    .map_err(|_| ApiError::validation_failed("coach_id が UUID でない"))
            })
            .transpose()?,
        has_unanswered: params.has_unanswered.unwrap_or(false),
        has_pending_video: params.has_pending_video.unwrap_or(false),
        has_unhandled_low_rated: params.has_unhandled_low_rated.unwrap_or(false),
    };
    let students = state
        .list_students
        .execute(&query)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(StudentListBody {
        students: students
            .iter()
            .map(student_body)
            .collect::<Result<_, _>>()?,
    }))
}

/// 本部が見る生徒の一覧（admin-web の `GET /students`）のルーター。組み立て用のクレートは他の機能のルーターと
/// `merge` する（同じ検証器を渡す）。
///
/// `GET /students` は本部だけ（コーチ・生徒は 403）。
pub fn students_router<R, C, B, K, N, U, P, W, L, T>(
    list_students: ListStudents<R, C, B, K, N, U, P, W, L>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    C: StudentContractLookup + 'static,
    B: StudentLastBookingLookup + 'static,
    K: StudentCoachBookingLookup + 'static,
    N: StudentRecentBookingCountLookup + 'static,
    U: StudentUnansweredMessageThreadCountsLookup + 'static,
    P: StudentPendingVideoReviewCountsLookup + 'static,
    W: StudentLowRatedReviewCountsLookup + 'static,
    L: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/students",
            get(self::list_students::<R, C, B, K, N, U, P, W, L>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(StudentsState { list_students }))
        .layer(TokenVerification::layer(tokens))
}
