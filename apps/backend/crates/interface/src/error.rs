//! OpenAPI の `Error`（`code` と `message`）で返すエラー応答。429 には `Retry-After` を付け、`email_already_registered` の 409 には
//! ログインへの案内 `login_guidance` を足せる（student-app の `EmailAlreadyRegisteredError`）。

use std::{fmt, time::Duration};

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use domain::DomainError;
use serde::Serialize;

/// エラー応答。`code` は OpenAPI の `ErrorCode` の値。
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    /// `Retry-After` の秒数（429 だけ）
    retry_after: Option<u64>,
    /// ログインへの案内に載せるメールアドレス（`email_already_registered` の 409 だけ）
    login_guidance: Option<String>,
}

impl ApiError {
    /// 422 `validation_failed`
    pub fn validation_failed(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "validation_failed",
            message: message.into(),
            retry_after: None,
            login_guidance: None,
        }
    }

    /// 401 `unauthorized`（トークンがない、または無効）
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: message.into(),
            retry_after: None,
            login_guidance: None,
        }
    }

    /// 401 と、その理由の `code`（ログインの確認コードが違う・期限切れなど）
    pub fn unauthorized_because(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code,
            message: message.into(),
            retry_after: None,
            login_guidance: None,
        }
    }

    /// 429 `too_many_requests` と、次に試せるまでの `Retry-After`（秒。端数は切り上げる）
    pub fn too_many_requests(message: impl Into<String>, retry_after: Duration) -> Self {
        let seconds = retry_after.as_secs() + u64::from(retry_after.subsec_nanos() > 0);
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "too_many_requests",
            message: message.into(),
            retry_after: Some(seconds),
            login_guidance: None,
        }
    }

    /// 本文に、`email` でログインへ進む案内 `login_guidance`（`requestLoginCode` のパス付き）を足す。
    pub fn with_login_guidance(mut self, email: impl Into<String>) -> Self {
        self.login_guidance = Some(email.into());
        self
    }

    /// 403 `forbidden`（役割や受講権が操作に合わない）
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: message.into(),
            retry_after: None,
            login_guidance: None,
        }
    }

    /// 403 と、その理由の `code`（役割は合っているが受講権が操作に合わないなど）
    pub fn forbidden_because(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code,
            message: message.into(),
            retry_after: None,
            login_guidance: None,
        }
    }

    /// 404 `not_found`（対象が存在しない）
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: message.into(),
            retry_after: None,
            login_guidance: None,
        }
    }

    /// 409 と、その理由の `code`
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
            retry_after: None,
            login_guidance: None,
        }
    }

    /// 500。契約の `ErrorCode` に当たる値がないので `code` は `internal`（未知の値は汎用エラー）。
    ///
    /// `cause`（内部の理由）はクライアントに返さず、標準エラー出力に書く。`message` は固定の文言。
    pub fn internal(cause: impl fmt::Display) -> Self {
        eprintln!("internal error: {cause}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: INTERNAL_MESSAGE.to_string(),
            retry_after: None,
            login_guidance: None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn status(&self) -> StatusCode {
        self.status
    }

    #[cfg(test)]
    pub(crate) const fn code(&self) -> &'static str {
        self.code
    }

    #[cfg(test)]
    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

/// 500 の `message`。原因はクライアントに出さない。
const INTERNAL_MESSAGE: &str = "サーバー内部のエラー";

/// ログインの 1 段目の操作（student-app の `requestLoginCode`）のパス。
const REQUEST_LOGIN_CODE_PATH: &str = "/login-codes";

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    login_guidance: Option<LoginGuidanceBody<'a>>,
}

/// OpenAPI の `LoginGuidance`。
#[derive(Serialize)]
struct LoginGuidanceBody<'a> {
    email: &'a str,
    request_login_code_path: &'static str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            code: self.code,
            message: &self.message,
            login_guidance: self
                .login_guidance
                .as_deref()
                .map(|email| LoginGuidanceBody {
                    email,
                    request_login_code_path: REQUEST_LOGIN_CODE_PATH,
                }),
        };
        let mut response = (self.status, Json(body)).into_response();
        if let Some(seconds) = self.retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}

/// 不変条件の違反は 422 `validation_failed`。文言は [`crate::message::domain_error`]。
impl From<DomainError> for ApiError {
    fn from(error: DomainError) -> Self {
        Self::validation_failed(crate::message::domain_error(&error))
    }
}
