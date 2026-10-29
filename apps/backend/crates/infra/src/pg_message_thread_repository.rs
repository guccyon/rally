//! スレッドとメッセージの保存先（Postgres の `messaging.threads`・`messages`）。

use std::collections::HashMap;

use application::{
    Unavailable,
    async_lesson::{video_id_uuid, video_review_id_uuid},
    identity::account_id_uuid,
    messaging::{
        MessageThreadRepository, UpdateMessageThreadError, message_id_uuid, message_thread_id_uuid,
    },
};
use domain::{
    DomainError,
    identity::AccountId,
    messaging::{
        Message, MessageBody, MessageContent, MessageId, MessageKind, MessageThread,
        MessageThreadId,
    },
};
use sqlx::{
    PgExecutor, PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    messaging_values::{
        message_author_from_value, message_author_value, message_kind_from_value,
        message_kind_value,
    },
    pg_support::timestamp,
};

/// [`MessageThreadRepository`] の Postgres 実装。
///
/// 同じ生徒とコーチのスレッドは 1 つ（`threads_student_id_coach_id_key`。開くときは `ON CONFLICT DO NOTHING`）。
/// 送信・既読はスレッドの行のロック（`FOR UPDATE`）の中で読み取りから保存までを行う。メッセージは送った順
/// （`seq`）に読む。メッセージの種類（`kind`）ごとの中身は、本文（`body`）と動画・添削の ID（`video_id` / `review_id`）の列に
/// 分けて持つ（形は `messages_kind_shape`）。
#[derive(Debug, Clone)]
pub struct PgMessageThreadRepository {
    pool: PgPool,
}

/// スレッドの列（thread_id, student_id, coach_id, opened_at）。
type ThreadRow = (Uuid, Uuid, Uuid, OffsetDateTime);

/// メッセージの列（thread_id, message_id, sender_role, kind, body, video_id, review_id, sent_at, read_at）。
type MessageRow = (
    Uuid,
    Uuid,
    String,
    String,
    Option<String>,
    Option<Uuid>,
    Option<Uuid>,
    OffsetDateTime,
    Option<OffsetDateTime>,
);

const MESSAGES_OF: &str =
    "SELECT thread_id, message_id, sender_role, kind, body, video_id, review_id,
            sent_at, read_at
     FROM messaging.messages
     WHERE thread_id = ANY($1)
     ORDER BY seq";

impl PgMessageThreadRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl MessageThreadRepository for PgMessageThreadRepository {
    fn next_thread_id(&self) -> MessageThreadId {
        message_thread_id_uuid::from_uuid(Uuid::new_v4())
    }

    fn next_message_id(&self) -> MessageId {
        message_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn open(&self, thread: MessageThread) -> Result<MessageThread, Unavailable> {
        sqlx::query(
            "INSERT INTO messaging.threads (thread_id, student_id, coach_id, opened_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT ON CONSTRAINT threads_student_id_coach_id_key DO NOTHING",
        )
        .bind(message_thread_id_uuid::to_uuid(thread.id()))
        .bind(account_id_uuid::to_uuid(thread.student_id()))
        .bind(account_id_uuid::to_uuid(thread.coach_id()))
        .bind(timestamp(thread.opened_at()))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        let row: ThreadRow = sqlx::query_as(
            "SELECT thread_id, student_id, coach_id, opened_at
             FROM messaging.threads
             WHERE student_id = $1 AND coach_id = $2",
        )
        .bind(account_id_uuid::to_uuid(thread.student_id()))
        .bind(account_id_uuid::to_uuid(thread.coach_id()))
        .fetch_one(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        let messages = messages_of(&self.pool, &[row.0])
            .await
            .map_err(Unavailable::new)?;
        assemble(vec![row], messages).map(|mut threads| threads.remove(0))
    }

    async fn find(&self, thread_id: MessageThreadId) -> Result<Option<MessageThread>, Unavailable> {
        let row: Option<ThreadRow> = sqlx::query_as(
            "SELECT thread_id, student_id, coach_id, opened_at
             FROM messaging.threads
             WHERE thread_id = $1",
        )
        .bind(message_thread_id_uuid::to_uuid(thread_id))
        .fetch_optional(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let messages = messages_of(&self.pool, &[row.0])
            .await
            .map_err(Unavailable::new)?;
        assemble(vec![row], messages).map(|mut threads| threads.pop())
    }

    async fn list_for(&self, account_id: AccountId) -> Result<Vec<MessageThread>, Unavailable> {
        let rows: Vec<ThreadRow> = sqlx::query_as(
            "SELECT thread_id, student_id, coach_id, opened_at
             FROM messaging.threads
             WHERE student_id = $1 OR coach_id = $1",
        )
        .bind(account_id_uuid::to_uuid(account_id))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        let ids: Vec<Uuid> = rows.iter().map(|row| row.0).collect();
        let messages = messages_of(&self.pool, &ids)
            .await
            .map_err(Unavailable::new)?;
        assemble(rows, messages)
    }

    async fn update<F>(
        &self,
        thread_id: MessageThreadId,
        update: F,
    ) -> Result<MessageThread, UpdateMessageThreadError>
    where
        F: FnOnce(&mut MessageThread) -> Result<(), DomainError> + Send,
    {
        let unavailable =
            |e: sqlx::Error| UpdateMessageThreadError::Unavailable(Unavailable::new(e));
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let row: Option<ThreadRow> = sqlx::query_as(
            "SELECT thread_id, student_id, coach_id, opened_at
             FROM messaging.threads
             WHERE thread_id = $1
             FOR UPDATE",
        )
        .bind(message_thread_id_uuid::to_uuid(thread_id))
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?;
        let row = row.ok_or(UpdateMessageThreadError::NotFound)?;
        let messages = messages_of(&mut *tx, &[row.0]).await.map_err(unavailable)?;
        let before = assemble(vec![row], messages)
            .map_err(UpdateMessageThreadError::Unavailable)?
            .remove(0);
        let mut thread = before.clone();
        update(&mut thread).map_err(UpdateMessageThreadError::Rejected)?;

        // スレッドが変わるのは、メッセージが足されるか、既読が付くかだけ（本文・送信時刻は変わらない）
        let read_before: HashMap<MessageId, Option<OffsetDateTime>> = before
            .messages()
            .iter()
            .map(|m| (m.id(), m.read_at().map(timestamp)))
            .collect();
        for message in thread.messages() {
            let read_at = message.read_at().map(timestamp);
            match read_before.get(&message.id()) {
                None => {
                    let content = message.content();
                    sqlx::query(
                        "INSERT INTO messaging.messages
                             (message_id, thread_id, sender_role, kind, body, video_id, review_id,
                              sent_at, read_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                    )
                    .bind(message_id_uuid::to_uuid(message.id()))
                    .bind(message_thread_id_uuid::to_uuid(thread.id()))
                    .bind(message_author_value(message.author()))
                    .bind(message_kind_value(content.kind()))
                    .bind(content.body().map(MessageBody::as_str))
                    .bind(content.video_id().map(video_id_uuid::to_uuid))
                    .bind(content.review_id().map(video_review_id_uuid::to_uuid))
                    .bind(timestamp(message.sent_at()))
                    .bind(read_at)
                    .execute(&mut *tx)
                    .await
                    .map_err(unavailable)?;
                }
                Some(previous) if *previous != read_at => {
                    sqlx::query("UPDATE messaging.messages SET read_at = $2 WHERE message_id = $1")
                        .bind(message_id_uuid::to_uuid(message.id()))
                        .bind(read_at)
                        .execute(&mut *tx)
                        .await
                        .map_err(unavailable)?;
                }
                Some(_) => {}
            }
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(thread)
    }
}

/// `thread_ids` のスレッドのメッセージ（送った順）。
async fn messages_of<'e>(
    executor: impl PgExecutor<'e>,
    thread_ids: &[Uuid],
) -> Result<Vec<MessageRow>, sqlx::Error> {
    sqlx::query_as(MESSAGES_OF)
        .bind(thread_ids)
        .fetch_all(executor)
        .await
}

/// スレッドの行と、それらのメッセージの行（送った順）からスレッドを組み立てる（スレッドの行の順）。
fn assemble(
    rows: Vec<ThreadRow>,
    messages: Vec<MessageRow>,
) -> Result<Vec<MessageThread>, Unavailable> {
    let mut by_thread: HashMap<Uuid, Vec<Message>> = HashMap::new();
    for (thread_id, message_id, sender, kind, body, video_id, review_id, sent_at, read_at) in
        messages
    {
        let author = message_author_from_value(&sender)
            .ok_or_else(|| Unavailable::new(format!("未知の sender_role: {sender}")))?;
        let message = Message::restore(
            message_id_uuid::from_uuid(message_id),
            author,
            content(&kind, body, video_id, review_id)?,
            sent_at.into(),
            read_at.map(Into::into),
        )
        .map_err(Unavailable::new)?;
        by_thread.entry(thread_id).or_default().push(message);
    }
    rows.into_iter()
        .map(|(thread_id, student_id, coach_id, opened_at)| {
            MessageThread::restore(
                message_thread_id_uuid::from_uuid(thread_id),
                account_id_uuid::from_uuid(student_id),
                account_id_uuid::from_uuid(coach_id),
                opened_at.into(),
                by_thread.remove(&thread_id).unwrap_or_default(),
            )
            .map_err(Unavailable::new)
        })
        .collect()
}

/// メッセージの種類 `kind` と中身の列から [`MessageContent`] を組み立てる。種類に要る列が欠けていれば失敗する
/// （DB の `messages_kind_shape` が守るので、起きるのは壊れた行だけ）。
fn content(
    kind: &str,
    body: Option<String>,
    video_id: Option<Uuid>,
    review_id: Option<Uuid>,
) -> Result<MessageContent, Unavailable> {
    let kind = message_kind_from_value(kind)
        .ok_or_else(|| Unavailable::new(format!("未知の kind: {kind}")))?;
    let missing =
        |column: &str| Unavailable::new(format!("{kind:?} のメッセージに {column} がない"));
    let video_id = || {
        video_id
            .map(video_id_uuid::from_uuid)
            .ok_or_else(|| missing("video_id"))
    };
    match kind {
        MessageKind::Text => {
            let body = body.ok_or_else(|| missing("body"))?;
            Ok(MessageContent::Text(
                MessageBody::new(body).map_err(Unavailable::new)?,
            ))
        }
        MessageKind::VideoReviewRequest => Ok(MessageContent::VideoReviewRequest {
            video_id: video_id()?,
        }),
        MessageKind::VideoReviewReturn => Ok(MessageContent::VideoReviewReturn {
            video_id: video_id()?,
            review_id: review_id
                .map(video_review_id_uuid::from_uuid)
                .ok_or_else(|| missing("review_id"))?,
        }),
    }
}
