//! OpenAPI の `Error`（`code` と `message`）で返すエラー応答。

use std::fmt;

use axum::{
    Json,
    http::StatusCode,
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
}

impl ApiError {
    /// 422 `validation_failed`
    pub fn validation_failed(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "validation_failed",
            message: message.into(),
        }
    }

    /// 401 `unauthorized`（トークンがない、または無効）
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: message.into(),
        }
    }

    /// 403 `forbidden`（役割や受講権が操作に合わない）
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: message.into(),
        }
    }

    /// 409 と、その理由の `code`
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
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
        }
    }
}

/// 500 の `message`。原因はクライアントに出さない。
const INTERNAL_MESSAGE: &str = "サーバー内部のエラー";

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            code: self.code,
            message: &self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

/// 不変条件の違反は 422 `validation_failed`。文言は [`crate::message::domain_error`]。
impl From<DomainError> for ApiError {
    fn from(error: DomainError) -> Self {
        Self::validation_failed(crate::message::domain_error(&error))
    }
}
