//! `/message-threads`。student-app と admin-web（コーチ）の、生徒と担当コーチの 1 対 1 のチャット
//! （`listMessageThreads`・`listMessages`・`sendMessage`・`markMessageThreadRead`）。両方のアプリで同じ操作・同じ形。
//!
//! 役割では絞らず、スレッドの当事者（生徒・担当コーチ）かをユースケースが確かめる。当事者でないアカウントには
//! スレッドがあることも知らせない（404）。生徒がチャットの送り放題（月額）を持たなければ送信は 403
//! `chat_not_entitled`。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    domain_event::DomainEventPublisher,
    identity::account_id_uuid,
    messaging::{
        ChatGate, ListMessageThreads, ListMessageThreadsError, ListMessageThreadsQuery,
        ListMessages, ListMessagesError, ListMessagesQuery, MarkMessageThreadRead,
        MarkMessageThreadReadCommand, MarkMessageThreadReadError, MessageThreadRepository,
        SendMessage, SendMessageCommand, SendMessageError, message_id_uuid, message_thread_id_uuid,
    },
};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
};
use domain::{
    DomainError,
    identity::AccountId,
    messaging::{Message, MessageBody, MessageThread, MessageThreadId},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedAccount,
    contract::{date_time_to_contract, message_sender_to_contract},
    error::ApiError,
    message,
};

/// `/message-threads` のハンドラーが使うユースケース。
pub struct MessagingState<T, G, A, P, C> {
    pub list_message_threads: ListMessageThreads<T>,
    pub list_messages: ListMessages<T>,
    pub send_message: SendMessage<T, G, A, P, C>,
    pub mark_message_thread_read: MarkMessageThreadRead<T, C>,
}

/// ハンドラーが共有する [`MessagingState`]。
type Shared<T, G, A, P, C> = Arc<MessagingState<T, G, A, P, C>>;

/// student-app・admin-web の `SendMessageRequest`。
#[derive(Deserialize)]
pub struct SendMessageRequest {
    body: String,
}

/// student-app・admin-web の `Message`（domain の [`MessageBody`] と名前が重ならないよう `Json` を付ける）。
#[derive(Serialize)]
pub struct MessageJson {
    message_id: String,
    thread_id: String,
    sender_id: String,
    sender_role: &'static str,
    body: String,
    sent_at: String,
    read_at: Option<String>,
}

/// student-app・admin-web の `MessageThread`。`unread_count` は呼んだアカウントにとっての未読の数。
#[derive(Serialize)]
pub struct MessageThreadJson {
    thread_id: String,
    student_id: String,
    coach_id: String,
    opened_at: String,
    last_message: Option<MessageJson>,
    unread_count: usize,
}

/// student-app・admin-web の `MessageThreadList`。
#[derive(Serialize)]
pub struct MessageThreadListJson {
    threads: Vec<MessageThreadJson>,
}

/// student-app・admin-web の `MessageList`。
#[derive(Serialize)]
pub struct MessageListJson {
    thread_id: String,
    messages: Vec<MessageJson>,
}

/// `GET /message-threads`。認証したアカウントが当事者のスレッドを、最後の出来事の新しい順に返す。
pub async fn list_message_threads<T, G, A, P, C>(
    State(state): State<Shared<T, G, A, P, C>>,
    account: AuthenticatedAccount,
) -> Result<Json<MessageThreadListJson>, ApiError>
where
    T: MessageThreadRepository,
    G: ChatGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    let threads = state
        .list_message_threads
        .execute(ListMessageThreadsQuery {
            account_id: account.account_id(),
        })
        .await
        .map_err(|error| match error {
            ListMessageThreadsError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(MessageThreadListJson {
        threads: threads
            .iter()
            .map(|thread| thread_json(thread, account.account_id()))
            .collect::<Result<_, _>>()?,
    }))
}

/// `GET /message-threads/{thread_id}/messages`。スレッドの当事者がメッセージを送った順に読む（既読にはしない）。
pub async fn list_messages<T, G, A, P, C>(
    State(state): State<Shared<T, G, A, P, C>>,
    account: AuthenticatedAccount,
    Path(thread_id): Path<String>,
) -> Result<Json<MessageListJson>, ApiError>
where
    T: MessageThreadRepository,
    G: ChatGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    let thread = state
        .list_messages
        .execute(ListMessagesQuery {
            account_id: account.account_id(),
            thread_id: parse_thread_id(&thread_id)?,
        })
        .await
        .map_err(|error| match error {
            // 当事者でないスレッドは、あることも知らせない
            ListMessagesError::ThreadNotFound
            | ListMessagesError::Invalid(DomainError::NotAThreadParticipant) => thread_not_found(),
            ListMessagesError::Invalid(error) => ApiError::internal(error),
            ListMessagesError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(MessageListJson {
        thread_id: message_thread_id_uuid::format(thread.id()),
        messages: thread
            .messages()
            .iter()
            .map(|message| message_json(&thread, message))
            .collect::<Result<_, _>>()?,
    }))
}

/// `POST /message-threads/{thread_id}/messages`。スレッドの当事者がメッセージを送る。月額の生徒は送り放題。
pub async fn send_message<T, G, A, P, C>(
    State(state): State<Shared<T, G, A, P, C>>,
    account: AuthenticatedAccount,
    Path(thread_id): Path<String>,
    request: Result<Json<SendMessageRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<MessageJson>), ApiError>
where
    T: MessageThreadRepository,
    G: ChatGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    let thread_id = parse_thread_id(&thread_id)?;
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let command = SendMessageCommand {
        sender_id: account.account_id(),
        thread_id,
        body: MessageBody::new(request.body)?,
    };

    let sent = state
        .send_message
        .execute(command)
        .await
        .map_err(|error| match error {
            SendMessageError::ThreadNotFound
            | SendMessageError::Invalid(DomainError::NotAThreadParticipant) => thread_not_found(),
            SendMessageError::Invalid(error @ DomainError::ChatNotAllowed) => {
                ApiError::forbidden_because("chat_not_entitled", message::domain_error(&error))
            }
            SendMessageError::Invalid(error) => ApiError::internal(error),
            SendMessageError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok((
        StatusCode::CREATED,
        Json(message_json(&sent.thread, &sent.message)?),
    ))
}

/// `POST /message-threads/{thread_id}/read`。スレッドの当事者が、相手から届いたメッセージをすべて既読にする。
pub async fn mark_message_thread_read<T, G, A, P, C>(
    State(state): State<Shared<T, G, A, P, C>>,
    account: AuthenticatedAccount,
    Path(thread_id): Path<String>,
) -> Result<Json<MessageThreadJson>, ApiError>
where
    T: MessageThreadRepository,
    G: ChatGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    let thread = state
        .mark_message_thread_read
        .execute(MarkMessageThreadReadCommand {
            reader_id: account.account_id(),
            thread_id: parse_thread_id(&thread_id)?,
        })
        .await
        .map_err(|error| match error {
            MarkMessageThreadReadError::ThreadNotFound
            | MarkMessageThreadReadError::Invalid(DomainError::NotAThreadParticipant) => {
                thread_not_found()
            }
            MarkMessageThreadReadError::Invalid(error) => ApiError::internal(error),
            MarkMessageThreadReadError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(thread_json(&thread, account.account_id())?))
}

/// 契約に 422 はない（パスの ID）ので、UUID でない ID はないスレッドとして扱う。
fn parse_thread_id(text: &str) -> Result<MessageThreadId, ApiError> {
    message_thread_id_uuid::parse(text).map_err(|_| thread_not_found())
}

fn thread_not_found() -> ApiError {
    ApiError::not_found("スレッドがない")
}

fn thread_json(thread: &MessageThread, reader: AccountId) -> Result<MessageThreadJson, ApiError> {
    Ok(MessageThreadJson {
        thread_id: message_thread_id_uuid::format(thread.id()),
        student_id: account_id_uuid::format(thread.student_id()),
        coach_id: account_id_uuid::format(thread.coach_id()),
        opened_at: date_time_to_contract(thread.opened_at())?,
        last_message: thread
            .last_message()
            .map(|message| message_json(thread, message))
            .transpose()?,
        unread_count: thread.unread_count_for(reader),
    })
}

fn message_json(thread: &MessageThread, message: &Message) -> Result<MessageJson, ApiError> {
    Ok(MessageJson {
        message_id: message_id_uuid::format(message.id()),
        thread_id: message_thread_id_uuid::format(thread.id()),
        sender_id: account_id_uuid::format(thread.account_of(message.sender())),
        sender_role: message_sender_to_contract(message.sender()),
        body: message.body().as_str().to_string(),
        sent_at: date_time_to_contract(message.sent_at())?,
        read_at: message.read_at().map(date_time_to_contract).transpose()?,
    })
}
