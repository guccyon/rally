//! `/students`。admin-web の `GET /students`（`listStudents`。本部が見る生徒の一覧）。本部だけが呼べる。

use std::sync::Arc;

use application::identity::{
    AccountRepository, ListStudents, StudentContractLookup, StudentLastBookingLookup,
    StudentOverview, account_id_uuid,
};
use axum::{Json, extract::State};
use serde::Serialize;

use crate::{
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

/// `GET /students`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。生徒をすべて、登録の
/// 新しい順で返す（ゴルフの生徒も載せる）。
pub async fn list_students<R, C, B>(
    State(state): State<Arc<StudentsState<R, C, B>>>,
) -> Result<Json<StudentListBody>, ApiError>
where
    R: AccountRepository,
    C: StudentContractLookup,
    B: StudentLastBookingLookup,
{
    let students = state
        .list_students
        .execute()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(StudentListBody {
        students: students
            .iter()
            .map(student_body)
            .collect::<Result<_, _>>()?,
    }))
}
