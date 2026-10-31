//! messaging（メッセージ）の不変条件の違反（[`MessagingError`]）を、OpenAPI の `Error` の応答に写す。
//!
//! 文言と HTTP の状態・`code` はここだけで決める（`/message-threads` のハンドラーは変種を個別に拾わない）。
//!
//! - 当事者でない（`NotAThreadParticipant`）は、スレッドがあることも知らせない 404 `not_found`（ないスレッドと同じ）
//! - 生徒がチャットの送り放題を持たない（`ChatNotAllowed`）は 403 `chat_not_entitled`
//! - 本文の違反（`EmptyMessageBody`・`MessageBodyTooLong`）は 422 `validation_failed`
//! - それ以外は、ハンドラーが組み立てる送信・既読では起きない（保存した値の破損か時計の巻き戻り）。契約に当たる応答が
//!   ないので 500（理由はクライアントに返さない）

use domain::messaging::MessagingError;

use crate::error::ApiError;

/// ないスレッド（当事者でないスレッドも同じ）の 404。
pub(crate) fn thread_not_found() -> ApiError {
    ApiError::not_found("スレッドがない")
}

impl From<MessagingError> for ApiError {
    fn from(error: MessagingError) -> Self {
        match error {
            MessagingError::NotAThreadParticipant => thread_not_found(),
            MessagingError::ChatNotAllowed => Self::forbidden_because(
                "chat_not_entitled",
                "チャットを送れるのは月額契約（送り放題）の生徒だけ",
            ),
            MessagingError::EmptyMessageBody => Self::validation_failed("メッセージの本文が空"),
            MessagingError::MessageBodyTooLong { max } => {
                Self::validation_failed(format!("メッセージの本文は {max} 文字まで"))
            }
            MessagingError::MessageThreadWithOneself
            | MessagingError::MessageSentBeforeLastActivity
            | MessagingError::MessageReadBeforeSent
            | MessagingError::MessageAuthorMismatchesKind
            | MessagingError::MessageKindNotSendable => Self::internal(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;

    #[test]
    fn maps_each_messaging_error_to_a_status_and_code() {
        for (error, status, code, message) in [
            (
                MessagingError::NotAThreadParticipant,
                StatusCode::NOT_FOUND,
                "not_found",
                "スレッドがない",
            ),
            (
                MessagingError::ChatNotAllowed,
                StatusCode::FORBIDDEN,
                "chat_not_entitled",
                "チャットを送れるのは月額契約（送り放題）の生徒だけ",
            ),
            (
                MessagingError::EmptyMessageBody,
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "メッセージの本文が空",
            ),
            (
                MessagingError::MessageBodyTooLong { max: 2000 },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "メッセージの本文は 2000 文字まで",
            ),
            (
                MessagingError::MessageThreadWithOneself,
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "サーバー内部のエラー",
            ),
            (
                MessagingError::MessageSentBeforeLastActivity,
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "サーバー内部のエラー",
            ),
            (
                MessagingError::MessageReadBeforeSent,
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "サーバー内部のエラー",
            ),
            (
                MessagingError::MessageAuthorMismatchesKind,
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "サーバー内部のエラー",
            ),
            (
                MessagingError::MessageKindNotSendable,
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "サーバー内部のエラー",
            ),
        ] {
            let api_error = ApiError::from(error.clone());
            assert_eq!(
                (api_error.status(), api_error.code(), api_error.message()),
                (status, code, message),
                "{error:?}"
            );
        }
    }
}
