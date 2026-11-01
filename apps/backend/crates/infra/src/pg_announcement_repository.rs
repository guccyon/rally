//! 本部からのお知らせの送った記録の保存先（Postgres の `notification.announcements`）。

use std::{
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};

use application::{
    Unavailable,
    identity::account_id_uuid,
    notification::{AnnouncementRepository, announcement_id_uuid},
};
use domain::notification::{
    Announcement, AnnouncementBody, AnnouncementId, AnnouncementTitle, AnnouncementWithdrawal,
};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    identity_values::{sport_from_value, sport_value},
    pg_support::timestamp,
};

/// [`AnnouncementRepository`] の Postgres 実装。
///
/// 送った記録 1 件で 1 行。対象のスポーツは `identity.accounts.sport` と同じ値で、生徒全員なら NULL。新しい順は送った
/// 時刻（`sent_at`）と残した順（`seq`）で並べる。読んだ行は domain の [`Announcement::new`] に通して組み立てる
/// （題・本文・受け取った生徒の数が不変条件に合わなければ障害）。取り下げは同じ行の `withdrawn_by`・`withdrawn_at`
/// （取り下げていなければどちらも NULL）。
#[derive(Debug, Clone)]
pub struct PgAnnouncementRepository {
    pool: PgPool,
}

impl PgAnnouncementRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// 送った記録の列（announcement_id, sender_id, title, body, sport, recipient_count, sent_at, withdrawn_by,
/// withdrawn_at）。
type AnnouncementRow = (
    Uuid,
    Uuid,
    String,
    String,
    Option<String>,
    i32,
    OffsetDateTime,
    Option<Uuid>,
    Option<OffsetDateTime>,
);

/// ID が `$1` の送った記録。
const ANNOUNCEMENT: &str =
    "SELECT announcement_id, sender_id, title, body, sport, recipient_count, sent_at,
            withdrawn_by, withdrawn_at
     FROM notification.announcements
     WHERE announcement_id = $1";

/// 送った記録すべて（新しい順）。
const ALL_ANNOUNCEMENTS: &str =
    "SELECT announcement_id, sender_id, title, body, sport, recipient_count, sent_at,
            withdrawn_by, withdrawn_at
     FROM notification.announcements
     ORDER BY sent_at DESC, seq DESC";

/// ID が `$1` のどれかの送った記録。
const ANNOUNCEMENTS_OF: &str =
    "SELECT announcement_id, sender_id, title, body, sport, recipient_count, sent_at,
            withdrawn_by, withdrawn_at
     FROM notification.announcements
     WHERE announcement_id = ANY($1)";

impl AnnouncementRepository for PgAnnouncementRepository {
    fn next_id(&self) -> AnnouncementId {
        announcement_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn insert(&self, announcement: Announcement) -> Result<(), Unavailable> {
        let recipient_count =
            i32::try_from(announcement.recipient_count()).map_err(Unavailable::new)?;
        sqlx::query(
            "INSERT INTO notification.announcements
                 (announcement_id, sender_id, title, body, sport, recipient_count, sent_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(announcement_id_uuid::to_uuid(announcement.id()))
        .bind(account_id_uuid::to_uuid(announcement.sender_id()))
        .bind(announcement.title().as_str())
        .bind(announcement.body().as_str())
        .bind(announcement.sport().map(sport_value))
        .bind(recipient_count)
        .bind(timestamp(announcement.sent_at()))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn find(&self, id: AnnouncementId) -> Result<Option<Announcement>, Unavailable> {
        let row: Option<AnnouncementRow> = sqlx::query_as(ANNOUNCEMENT)
            .bind(announcement_id_uuid::to_uuid(id))
            .fetch_optional(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        row.map(announcement_from_row).transpose()
    }

    async fn update(&self, announcement: Announcement) -> Result<(), Unavailable> {
        let withdrawal = announcement.withdrawal();
        // 最初の取り下げを上書きしない（同時に取り下げても先に書いたほうが残る）
        sqlx::query(
            "UPDATE notification.announcements
             SET withdrawn_by = $2, withdrawn_at = $3
             WHERE announcement_id = $1 AND withdrawn_at IS NULL",
        )
        .bind(announcement_id_uuid::to_uuid(announcement.id()))
        .bind(withdrawal.map(|w| account_id_uuid::to_uuid(w.withdrawn_by())))
        .bind(withdrawal.map(|w| timestamp(w.withdrawn_at())))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn find_all(&self) -> Result<Vec<Announcement>, Unavailable> {
        let rows: Vec<AnnouncementRow> = sqlx::query_as(ALL_ANNOUNCEMENTS)
            .fetch_all(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        rows.into_iter().map(announcement_from_row).collect()
    }

    async fn find_by_ids(
        &self,
        ids: &BTreeSet<AnnouncementId>,
    ) -> Result<BTreeMap<AnnouncementId, Announcement>, Unavailable> {
        let ids: Vec<Uuid> = ids
            .iter()
            .copied()
            .map(announcement_id_uuid::to_uuid)
            .collect();
        let rows: Vec<AnnouncementRow> = sqlx::query_as(ANNOUNCEMENTS_OF)
            .bind(&ids)
            .fetch_all(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        rows.into_iter()
            .map(|row| announcement_from_row(row).map(|a| (a.id(), a)))
            .collect()
    }
}

fn announcement_from_row(row: AnnouncementRow) -> Result<Announcement, Unavailable> {
    let (id, sender_id, title, body, sport, recipient_count, sent_at, withdrawn_by, withdrawn_at) =
        row;
    let withdrawal = match (withdrawn_by, withdrawn_at) {
        (Some(by), Some(at)) => Some(AnnouncementWithdrawal::new(
            account_id_uuid::from_uuid(by),
            SystemTime::from(at),
        )),
        (None, None) => None,
        _ => {
            return Err(Unavailable::new(
                "取り下げのアカウントと時刻の片方だけがある",
            ));
        }
    };
    let sport = sport
        .map(|value| {
            sport_from_value(&value)
                .ok_or_else(|| Unavailable::new(format!("未知の sport: {value}")))
        })
        .transpose()?;
    Announcement::new(
        announcement_id_uuid::from_uuid(id),
        account_id_uuid::from_uuid(sender_id),
        AnnouncementTitle::new(title).map_err(Unavailable::new)?,
        AnnouncementBody::new(body).map_err(Unavailable::new)?,
        sport,
        u32::try_from(recipient_count).map_err(Unavailable::new)?,
        SystemTime::from(sent_at),
    )
    .map(|announcement| announcement.with_withdrawal(withdrawal))
    .map_err(Unavailable::new)
}
