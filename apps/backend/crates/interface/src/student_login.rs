//! `POST /login-codes`（OpenAPI の `requestLoginCode`）と `POST /sessions`（`createSession`）。student-app の生徒のログイン
//! （メールの 6 桁の確認コード。docs/adr/0012-student-login-one-time-code.md）。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{
        AccountRepository, CreateSession, CreateSessionCommand, CreateSessionError,
        LoginCodeRepository, LoginCodeSender, RequestLoginCode, RequestLoginCodeError, TokenClaims,
        TokenIssuer, TokenVerifier,
    },
};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    routing::post,
};
use domain::identity::LoginCodeChallenge;
use serde::{Deserialize, Serialize};

use crate::{
    accounts::{RegisterAccountResponse, student_body},
    auth::TokenVerification,
    error::ApiError,
};

/// OpenAPI の `RequestLoginCodeRequest`。値の検証は domain で行う。
#[derive(Deserialize)]
pub struct RequestLoginCodeRequest {
    email: String,
}

/// OpenAPI の `RequestLoginCodeResponse`。登録の有無と役割で変えない。
#[derive(Serialize)]
pub struct RequestLoginCodeResponse {
    email: String,
    expires_in_seconds: u64,
    resend_after_seconds: u64,
}

/// OpenAPI の `CreateSessionRequest`。値の検証は domain で行う。
#[derive(Deserialize)]
pub struct CreateSessionRequest {
    email: String,
    code: String,
}

/// 生徒のログインのハンドラーが使うユースケースとトークンの発行。
pub struct StudentLoginState<R, S, C> {
    pub request_login_code: RequestLoginCode<R, S, C>,
    pub create_session: CreateSession<R, C>,
    pub tokens: Arc<dyn TokenIssuer>,
}

pub async fn request_login_code<R, S, C>(
    State(state): State<Arc<StudentLoginState<R, S, C>>>,
    request: Result<Json<RequestLoginCodeRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<RequestLoginCodeResponse>), ApiError>
where
    R: AccountRepository + LoginCodeRepository,
    S: LoginCodeSender,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let email = request.email.clone();

    state
        .request_login_code
        .execute(request.email)
        .await
        .map_err(|error| match error {
            RequestLoginCodeError::Invalid(error) => ApiError::from(error),
            RequestLoginCodeError::TooManyRequests { retry_after } => {
                ApiError::too_many_requests("確認コードを送り直せるまで待つ", retry_after)
            }
            RequestLoginCodeError::Unavailable(error) => ApiError::internal(error),
        })?;

    Ok((
        StatusCode::ACCEPTED,
        Json(RequestLoginCodeResponse {
            email,
            expires_in_seconds: LoginCodeChallenge::EXPIRES_IN.as_secs(),
            resend_after_seconds: LoginCodeChallenge::RESEND_INTERVAL.as_secs(),
        }),
    ))
}

pub async fn create_session<R, S, C>(
    State(state): State<Arc<StudentLoginState<R, S, C>>>,
    request: Result<Json<CreateSessionRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<RegisterAccountResponse>), ApiError>
where
    R: AccountRepository + LoginCodeRepository,
    S: LoginCodeSender,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;

    let account = state
        .create_session
        .execute(CreateSessionCommand {
            email: request.email,
            code: request.code,
        })
        .await
        .map_err(|error| match error {
            CreateSessionError::Invalid(error) => ApiError::from(error),
            CreateSessionError::CodeInvalid => ApiError::unauthorized_because(
                "login_code_invalid",
                "確認コードが違う、または送っていない",
            ),
            CreateSessionError::CodeExpired => {
                ApiError::unauthorized_because("login_code_expired", "確認コードの期限が過ぎた")
            }
            CreateSessionError::TooManyAttempts { retry_after } => ApiError::too_many_requests(
                "確認コードの間違いが上限に達した。送り直して新しいコードで試す",
                retry_after,
            ),
            CreateSessionError::Unavailable(error) => ApiError::internal(error),
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
        StatusCode::OK,
        Json(RegisterAccountResponse::new(student, access_token)),
    ))
}

/// 生徒のログイン（student-app の `POST /login-codes`・`POST /sessions`）のルーター。どちらも認証なしで呼べ、
/// `POST /sessions` はログインした生徒のアクセストークンを `tokens` で発行する（`registerAccount` と同じ）。
pub fn student_login_router<R, S, C, T>(
    request_login_code: RequestLoginCode<R, S, C>,
    create_session: CreateSession<R, C>,
    tokens: T,
) -> Router
where
    R: AccountRepository + LoginCodeRepository + 'static,
    S: LoginCodeSender + 'static,
    C: Clock + 'static,
    T: TokenIssuer + TokenVerifier + 'static,
{
    let tokens = Arc::new(tokens);
    Router::new()
        .route("/login-codes", post(self::request_login_code::<R, S, C>))
        .route("/sessions", post(self::create_session::<R, S, C>))
        .with_state(Arc::new(StudentLoginState {
            request_login_code,
            create_session,
            tokens: tokens.clone(),
        }))
        .layer(TokenVerification::layer(tokens))
}
