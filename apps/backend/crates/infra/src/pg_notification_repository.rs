//! お知らせの保存先（Postgres の `notification.notifications`）。

use std::{collections::BTreeMap, time::SystemTime};

use application::{
    Unavailable,
    async_lesson::{video_id_uuid, video_review_id_uuid},
    identity::account_id_uuid,
    messaging::{message_id_uuid, message_thread_id_uuid},
    notification::{NotificationRepository, announcement_id_uuid, notification_id_uuid},
    scheduling::booking_id_uuid,
};
use domain::{
    identity::AccountId,
    notification::{AnnouncementId, Notification, NotificationId, PushNotification},
};
use sqlx::{
    PgExecutor, PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    messaging_values::{message_sender_from_value, message_sender_value},
    notification_values::{
        ANNOUNCEMENT, BOOKING_CONFIRMED, BOOKING_RECEIVED, MESSAGE_RECEIVED,
        VIDEO_REVIEW_REQUESTED, VIDEO_REVIEW_RETURNED, push_notification_kind_value,
    },
    pg_support::timestamp,
};

/// [`NotificationRepository`] の Postgres 実装。
///
/// お知らせ 1 件で 1 行。プッシュ通知の中身は種類（`kind`）ごとの列に分けて持つ（形は `notifications_kind_shape`）。
/// 本部からのお知らせはコーチを持たない（`coach_id` は NULL）で、送った記録（`announcement_id`）を指す。
/// 同じプッシュ通知のお知らせは 1 件（`notifications_one_per_push` と、添削の依頼は `notifications_one_per_video_review_request`、
/// 本部からのお知らせは `notifications_one_per_announcement`。残すときは `ON CONFLICT DO NOTHING`）。新しい順は
/// 作った時刻（`created_at`）と残した順（`seq`）で並べる。既読は、受け取る人のまだ読んでいない行をロック（`FOR UPDATE`）
/// して domain の [`Notification::mark_read`] に通してから書く。本部からのお知らせごとの既読の数は、送った記録を指す
/// 行（`kind = 'announcement'`）のうち `read_at` のあるもの（上限の時刻があればそれまでのもの）を数える
/// （`notifications_one_per_announcement` の索引で引く）。
#[derive(Debug, Clone)]
pub struct PgNotificationRepository {
    pool: PgPool,
}

impl PgNotificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// お知らせの列（notification_id, kind, student_id, coach_id, booking_id, video_id, review_id, thread_id, message_id,
/// sender_role, announcement_id, created_at, read_at）。
type NotificationRow = (
    Uuid,
    String,
    Uuid,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<String>,
    Option<Uuid>,
    OffsetDateTime,
    Option<OffsetDateTime>,
);

/// `recipient_id = $1` のお知らせ（新しい順）。
const NOTIFICATIONS_OF: &str =
    "SELECT notification_id, kind, student_id, coach_id, booking_id, video_id, review_id,
            thread_id, message_id, sender_role, announcement_id, created_at, read_at
     FROM notification.notifications
     WHERE recipient_id = $1
     ORDER BY created_at DESC, seq DESC";

/// `recipient_id = $1` のまだ読んでいないお知らせ（新しい順）。行のロックを取る。
const UNREAD_NOTIFICATIONS_OF_FOR_UPDATE: &str =
    "SELECT notification_id, kind, student_id, coach_id, booking_id, video_id,
            review_id, thread_id, message_id, sender_role, announcement_id, created_at, read_at
     FROM notification.notifications
     WHERE recipient_id = $1 AND read_at IS NULL
     ORDER BY created_at DESC, seq DESC
     FOR UPDATE";

/// 本部からのお知らせ `$1` のどれかを指す（種類が `$3`）お知らせのうち、既読のものの数（本部からのお知らせごと）。
/// `$2` は `$1` と同じ並びの、数えに入れる既読の時刻の上限（NULL なら上限なし）。
const READ_COUNTS_OF_ANNOUNCEMENTS: &str = "SELECT n.announcement_id, count(*)
     FROM notification.notifications n
     JOIN unnest($1::uuid[], $2::timestamptz[]) AS c (announcement_id, read_until)
         ON n.announcement_id = c.announcement_id
     WHERE n.kind = $3 AND n.read_at IS NOT NULL
         AND (c.read_until IS NULL OR n.read_at <= c.read_until)
     GROUP BY n.announcement_id";

/// プッシュ通知の中身の列（booking_id, video_id, review_id, thread_id, message_id, sender_role, announcement_id）。
type PushColumns = (
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<&'static str>,
    Option<Uuid>,
);

impl NotificationRepository for PgNotificationRepository {
    fn next_id(&self) -> NotificationId {
        notification_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn insert(&self, notification: Notification) -> Result<(), Unavailable> {
        let push = notification.push();
        let (student_id, coach_id) = parties(&push);
        let (booking_id, video_id, review_id, thread_id, message_id, sender_role, announcement_id) =
            push_columns(&push);
        sqlx::query(
            "INSERT INTO notification.notifications
                 (notification_id, recipient_id, kind, student_id, coach_id, booking_id, video_id, review_id,
                  thread_id, message_id, sender_role, announcement_id, created_at, read_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
             ON CONFLICT DO NOTHING",
        )
        .bind(notification_id_uuid::to_uuid(notification.id()))
        .bind(account_id_uuid::to_uuid(notification.recipient()))
        .bind(push_notification_kind_value(&push))
        .bind(account_id_uuid::to_uuid(student_id))
        .bind(coach_id.map(account_id_uuid::to_uuid))
        .bind(booking_id)
        .bind(video_id)
        .bind(review_id)
        .bind(thread_id)
        .bind(message_id)
        .bind(sender_role)
        .bind(announcement_id)
        .bind(timestamp(notification.created_at()))
        .bind(notification.read_at().map(timestamp))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn find_by_recipient(
        &self,
        recipient: AccountId,
    ) -> Result<Vec<Notification>, Unavailable> {
        notifications_of(&self.pool, recipient, false).await
    }

    async fn mark_all_read(&self, recipient: AccountId, at: SystemTime) -> Result<(), Unavailable> {
        let mut tx = self.pool.begin().await.map_err(Unavailable::new)?;
        let unread = notifications_of(&mut *tx, recipient, true).await?;
        let read: Vec<Uuid> = unread
            .into_iter()
            .filter_map(|mut notification| {
                notification.mark_read(at);
                notification
                    .is_read()
                    .then(|| notification_id_uuid::to_uuid(notification.id()))
            })
            .collect();
        if !read.is_empty() {
            sqlx::query(
                "UPDATE notification.notifications SET read_at = $2
                 WHERE notification_id = ANY($1)",
            )
            .bind(&read)
            .bind(timestamp(at))
            .execute(&mut *tx)
            .await
            .map_err(Unavailable::new)?;
        }
        tx.commit().await.map_err(Unavailable::new)
    }

    async fn count_read_by_announcement(
        &self,
        read_until: &BTreeMap<AnnouncementId, Option<SystemTime>>,
    ) -> Result<BTreeMap<AnnouncementId, u32>, Unavailable> {
        let (ids, untils): (Vec<Uuid>, Vec<Option<OffsetDateTime>>) = read_until
            .iter()
            .map(|(id, until)| (announcement_id_uuid::to_uuid(*id), until.map(timestamp)))
            .unzip();
        let rows: Vec<(Uuid, i64)> = sqlx::query_as(READ_COUNTS_OF_ANNOUNCEMENTS)
            .bind(&ids)
            .bind(&untils)
            .bind(ANNOUNCEMENT)
            .fetch_all(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        rows.into_iter()
            .map(|(id, count)| {
                Ok((
                    announcement_id_uuid::from_uuid(id),
                    u32::try_from(count).map_err(Unavailable::new)?,
                ))
            })
            .collect()
    }
}

/// `recipient` が受け取るお知らせ（新しい順）。`unread_for_update` なら、まだ読んでいないものだけを行のロックを取って読む。
async fn notifications_of<'e>(
    executor: impl PgExecutor<'e>,
    recipient: AccountId,
    unread_for_update: bool,
) -> Result<Vec<Notification>, Unavailable> {
    let query = if unread_for_update {
        UNREAD_NOTIFICATIONS_OF_FOR_UPDATE
    } else {
        NOTIFICATIONS_OF
    };
    let rows: Vec<NotificationRow> = sqlx::query_as(query)
        .bind(account_id_uuid::to_uuid(recipient))
        .fetch_all(executor)
        .await
        .map_err(Unavailable::new)?;
    rows.into_iter().map(notification_from_row).collect()
}

fn notification_from_row(row: NotificationRow) -> Result<Notification, Unavailable> {
    let (
        id,
        kind,
        student_id,
        coach_id,
        booking_id,
        video_id,
        review_id,
        thread_id,
        message_id,
        sender_role,
        announcement_id,
        created_at,
        read_at,
    ) = row;
    let broken = || Unavailable::new(format!("お知らせ {id} の中身が種類 {kind} の形でない"));
    let student_id = account_id_uuid::from_uuid(student_id);
    if kind == ANNOUNCEMENT {
        let push = PushNotification::Announcement {
            student_id,
            announcement_id: announcement_id_uuid::from_uuid(announcement_id.ok_or_else(broken)?),
        };
        return restore(id, push, created_at, read_at);
    }
    let coach_id = account_id_uuid::from_uuid(coach_id.ok_or_else(broken)?);
    let push = match kind.as_str() {
        BOOKING_CONFIRMED => PushNotification::BookingConfirmed {
            student_id,
            coach_id,
            booking_id: booking_id_uuid::from_uuid(booking_id.ok_or_else(broken)?),
        },
        BOOKING_RECEIVED => PushNotification::BookingReceived {
            student_id,
            coach_id,
            booking_id: booking_id_uuid::from_uuid(booking_id.ok_or_else(broken)?),
        },
        VIDEO_REVIEW_REQUESTED => PushNotification::VideoReviewRequested {
            student_id,
            coach_id,
            video_id: video_id_uuid::from_uuid(video_id.ok_or_else(broken)?),
        },
        VIDEO_REVIEW_RETURNED => PushNotification::VideoReviewReturned {
            student_id,
            coach_id,
            video_id: video_id_uuid::from_uuid(video_id.ok_or_else(broken)?),
            review_id: video_review_id_uuid::from_uuid(review_id.ok_or_else(broken)?),
        },
        MESSAGE_RECEIVED => PushNotification::MessageReceived {
            student_id,
            coach_id,
            thread_id: message_thread_id_uuid::from_uuid(thread_id.ok_or_else(broken)?),
            message_id: message_id_uuid::from_uuid(message_id.ok_or_else(broken)?),
            sender: sender_role
                .as_deref()
                .and_then(message_sender_from_value)
                .ok_or_else(broken)?,
        },
        _ => return Err(Unavailable::new(format!("未知の kind: {kind}"))),
    };
    restore(id, push, created_at, read_at)
}

fn restore(
    id: Uuid,
    push: PushNotification,
    created_at: OffsetDateTime,
    read_at: Option<OffsetDateTime>,
) -> Result<Notification, Unavailable> {
    Notification::restore(
        notification_id_uuid::from_uuid(id),
        push,
        SystemTime::from(created_at),
        read_at.map(SystemTime::from),
    )
    .map_err(Unavailable::new)
}

/// プッシュ通知の当事者（生徒, コーチ）。本部からのお知らせはコーチを持たない。
fn parties(push: &PushNotification) -> (AccountId, Option<AccountId>) {
    match *push {
        PushNotification::BookingConfirmed {
            student_id,
            coach_id,
            ..
        }
        | PushNotification::BookingReceived {
            student_id,
            coach_id,
            ..
        }
        | PushNotification::VideoReviewRequested {
            student_id,
            coach_id,
            ..
        }
        | PushNotification::VideoReviewReturned {
            student_id,
            coach_id,
            ..
        }
        | PushNotification::MessageReceived {
            student_id,
            coach_id,
            ..
        } => (student_id, Some(coach_id)),
        PushNotification::Announcement { student_id, .. } => (student_id, None),
    }
}

fn push_columns(push: &PushNotification) -> PushColumns {
    match *push {
        PushNotification::BookingConfirmed { booking_id, .. }
        | PushNotification::BookingReceived { booking_id, .. } => (
            Some(booking_id_uuid::to_uuid(booking_id)),
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        PushNotification::VideoReviewRequested { video_id, .. } => (
            None,
            Some(video_id_uuid::to_uuid(video_id)),
            None,
            None,
            None,
            None,
            None,
        ),
        PushNotification::VideoReviewReturned {
            video_id,
            review_id,
            ..
        } => (
            None,
            Some(video_id_uuid::to_uuid(video_id)),
            Some(video_review_id_uuid::to_uuid(review_id)),
            None,
            None,
            None,
            None,
        ),
        PushNotification::MessageReceived {
            thread_id,
            message_id,
            sender,
            ..
        } => (
            None,
            None,
            None,
            Some(message_thread_id_uuid::to_uuid(thread_id)),
            Some(message_id_uuid::to_uuid(message_id)),
            Some(message_sender_value(sender)),
            None,
        ),
        PushNotification::Announcement {
            announcement_id, ..
        } => (
            None,
            None,
            None,
            None,
            None,
            None,
            Some(announcement_id_uuid::to_uuid(announcement_id)),
        ),
    }
}
