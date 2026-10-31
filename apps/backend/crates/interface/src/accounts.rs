//! `POST /accounts`（OpenAPI の `registerAccount`）。student-app の生徒の登録。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    identity::{
        AccountRepository, RegisterAccount, RegisterAccountCommand, RegisterAccountError,
        TokenClaims, TokenIssuer, TokenVerifier, account_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    routing::post,
};
use domain::identity::{Account, Role};
use serde::{Deserialize, Serialize};

use crate::{
    auth::TokenVerification,
    contract::{date_time_to_contract, sport_to_contract, student_app_sport},
    error::ApiError,
};

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

/// `POST /accounts` のハンドラーが使うユースケースとトークンの発行。
pub struct AccountsState<R, A, C> {
    pub register_account: RegisterAccount<R, A, C>,
    pub tokens: Arc<dyn TokenIssuer>,
}

pub async fn register_account<R, A, C>(
    State(state): State<Arc<AccountsState<R, A, C>>>,
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

    let account = state
        .register_account
        .execute(RegisterAccountCommand {
            email: request.email,
            display_name: request.display_name,
            role: Role::Student,
            sports: vec![sport],
        })
        .await
        .map_err(|error| match error {
            RegisterAccountError::Invalid(error) => ApiError::from(error),
            RegisterAccountError::EmailAlreadyRegistered => ApiError::conflict(
                "email_already_registered",
                "このメールアドレスはすでに登録されている",
            ),
            RegisterAccountError::Unavailable(error) => ApiError::internal(error),
        })?;

    let student = student_body(&account)?;
    let access_token = state
        .tokens
        .issue(TokenClaims {
            account_id: account.id(),
            role: account.role(),
        })
        .into_string();
    Ok((
        StatusCode::CREATED,
        Json(RegisterAccountResponse {
            student,
            access_token,
        }),
    ))
}

pub(crate) fn student_body(account: &Account) -> Result<StudentBody, ApiError> {
    let student = account
        .as_student()
        .ok_or_else(|| ApiError::internal("生徒のアカウントが生徒でない"))?;
    Ok(StudentBody {
        student_id: account_id_uuid::format(account.id()),
        display_name: account.display_name().as_str().to_string(),
        sport: sport_to_contract(student.sport()),
        created_at: date_time_to_contract(account.registered_at())?,
    })
}

/// 生徒の登録（student-app の `POST /accounts`）のルーター。認証なしで呼べ、登録した生徒のアクセストークンを `tokens` で
/// 発行する。組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
pub fn accounts_router<R, A, C, T>(register_account: RegisterAccount<R, A, C>, tokens: T) -> Router
where
    R: AccountRepository + 'static,
    A: AnalyticsSink + 'static,
    C: Clock + 'static,
    T: TokenIssuer + TokenVerifier + 'static,
{
    let tokens = Arc::new(tokens);
    Router::new()
        .route("/accounts", post(self::register_account::<R, A, C>))
        .with_state(Arc::new(AccountsState {
            register_account,
            tokens: tokens.clone(),
        }))
        .layer(TokenVerification::layer(tokens))
}
