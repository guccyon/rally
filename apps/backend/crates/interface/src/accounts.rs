//! `POST /accounts`（OpenAPI の `registerAccount`）。student-app の生徒の登録。

use std::{sync::Arc, time::SystemTime};

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    identity::{AccountRepository, RegisterAccount, RegisterAccountCommand, RegisterAccountError},
};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
};
use domain::identity::{Account, Role, Sport};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::ApiError;

/// OpenAPI の `RegisterAccountRequest`。値の検証は domain で行う。
#[derive(Deserialize)]
pub struct RegisterAccountRequest {
    email: String,
    display_name: String,
    sport: String,
}

/// OpenAPI の `RegisterAccountResponse`。
#[derive(Serialize)]
pub struct RegisterAccountResponse {
    student: StudentBody,
    access_token: String,
}

/// OpenAPI の `Student`。
#[derive(Serialize)]
pub struct StudentBody {
    student_id: String,
    display_name: String,
    sport: &'static str,
    created_at: String,
}

pub async fn register_account<R, A, C>(
    State(use_case): State<Arc<RegisterAccount<R, A, C>>>,
    request: Result<Json<RegisterAccountRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<RegisterAccountResponse>), ApiError>
where
    R: AccountRepository,
    A: AnalyticsSink,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let sport = student_app_sport(&request.sport)?;

    let account = use_case
        .execute(RegisterAccountCommand {
            email: request.email,
            display_name: request.display_name,
            role: Role::Student,
            sports: vec![sport],
        })
        .await
        .map_err(|error| match error {
            RegisterAccountError::Invalid(error) => ApiError::validation_failed(error.to_string()),
            RegisterAccountError::EmailAlreadyRegistered => ApiError::conflict(
                "email_already_registered",
                "このメールアドレスはすでに登録されている",
            ),
            RegisterAccountError::Unavailable(reason) => ApiError::internal(reason),
        })?;

    let student = student_body(&account)?;
    // T06（モックの認証）でトークンの発行・検証のポートに置き換える。
    let access_token = format!("mock-{}", student.student_id);
    Ok((
        StatusCode::CREATED,
        Json(RegisterAccountResponse {
            student,
            access_token,
        }),
    ))
}

/// student-app の `Sport`（ゴルフを除く 4 種類）。ゴルフは golf-app の範囲。
fn student_app_sport(value: &str) -> Result<Sport, ApiError> {
    match value.parse::<Sport>() {
        Ok(Sport::Golf) => Err(ApiError::validation_failed(
            "student-app はゴルフを扱わない（golf-app の範囲）",
        )),
        Ok(sport) => Ok(sport),
        Err(error) => Err(ApiError::validation_failed(error.to_string())),
    }
}

fn student_body(account: &Account) -> Result<StudentBody, ApiError> {
    let student = account
        .as_student()
        .ok_or_else(|| ApiError::internal("生徒として登録したアカウントが生徒でない"))?;
    Ok(StudentBody {
        student_id: account.id().to_string(),
        display_name: account.display_name().as_str().to_string(),
        sport: student.sport().as_str(),
        created_at: rfc3339(account.registered_at())?,
    })
}

fn rfc3339(at: SystemTime) -> Result<String, ApiError> {
    OffsetDateTime::from(at)
        .format(&Rfc3339)
        .map_err(|error| ApiError::internal(error.to_string()))
}
