//! `/students`。admin-web の `GET /students`（`listStudents`。本部が見る生徒の一覧）。本部だけが呼べる。

use std::sync::Arc;

use application::identity::{
    AccountRepository, ListStudents, StudentContractLookup, StudentLastBookingLookup,
    StudentOverview, TokenVerifier, account_id_uuid,
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::{identity::Role, profile::DisplayNameQuery};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{TokenVerification, require_role},
    contract::{
        contract_type_to_contract, date_time_to_contract, sport_to_contract,
        subscription_plan_to_contract,
    },
    error::ApiError,
};

/// `/students` のハンドラーが使うユースケース。
pub struct StudentsState<R, C, B> {
    pub list_students: ListStudents<R, C, B>,
}

/// admin-web の `listStudents` のクエリ。
#[derive(Deserialize)]
pub struct ListStudentsParams {
    q: Option<String>,
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
        registered_at: date_time_to_contract(student.registered_at)?,
    })
}

/// `GET /students`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。生徒を登録の新しい順で
/// 返す（ゴルフの生徒も載せる）。`q` があれば表示名に部分一致する生徒だけで、長すぎる `q` は 422。
pub async fn list_students<R, C, B>(
    State(state): State<Arc<StudentsState<R, C, B>>>,
    params: Result<Query<ListStudentsParams>, QueryRejection>,
) -> Result<Json<StudentListBody>, ApiError>
where
    R: AccountRepository,
    C: StudentContractLookup,
    B: StudentLastBookingLookup,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let name = params
        .q
        .as_deref()
        .map(DisplayNameQuery::new)
        .transpose()?
        .flatten();
    let students = state
        .list_students
        .execute(name.as_ref())
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
pub fn students_router<R, C, B, T>(list_students: ListStudents<R, C, B>, tokens: T) -> Router
where
    R: AccountRepository + 'static,
    C: StudentContractLookup + 'static,
    B: StudentLastBookingLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/students", get(self::list_students::<R, C, B>))
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(StudentsState { list_students }))
        .layer(TokenVerification::layer(tokens))
}
