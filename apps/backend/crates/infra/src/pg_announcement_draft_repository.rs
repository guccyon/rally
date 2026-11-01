//! 本部からのお知らせの下書きの保存先（Postgres の `notification.announcement_drafts`）。

use std::time::SystemTime;

use application::{
    Unavailable,
    identity::account_id_uuid,
    notification::{AnnouncementDraftRepository, announcement_draft_id_uuid},
};
use domain::notification::{AnnouncementDraft, AnnouncementDraftId};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    identity_values::{sport_from_value, sport_value},
    pg_support::timestamp,
};

/// [`AnnouncementDraftRepository`] の Postgres 実装。
///
/// 下書き 1 件で 1 行。対象のスポーツは `identity.accounts.sport` と同じ値で、生徒全員なら NULL。新しい順は最後に直した
/// 時刻（`updated_at`）と作った順（`seq`）で並べる。読んだ行は domain の [`AnnouncementDraft::new`] に通して組み立てる
/// （題・本文が上限を超えていれば障害）。送った下書きと消した下書きは行を消す。
#[derive(Debug, Clone)]
pub struct PgAnnouncementDraftRepository {
    pool: PgPool,
}

impl PgAnnouncementDraftRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// 下書きの列（announcement_draft_id, created_by, title, body, sport, created_at, updated_at）。
type DraftRow = (
    Uuid,
    Uuid,
    String,
    String,
    Option<String>,
    OffsetDateTime,
    OffsetDateTime,
);

/// ID が `$1` の下書き。
const DRAFT: &str =
    "SELECT announcement_draft_id, created_by, title, body, sport, created_at, updated_at
     FROM notification.announcement_drafts
     WHERE announcement_draft_id = $1";

/// 下書きすべて（最後に直した時刻の新しい順）。
const ALL_DRAFTS: &str =
    "SELECT announcement_draft_id, created_by, title, body, sport, created_at, updated_at
     FROM notification.announcement_drafts
     ORDER BY updated_at DESC, seq DESC";

impl AnnouncementDraftRepository for PgAnnouncementDraftRepository {
    fn next_id(&self) -> AnnouncementDraftId {
        announcement_draft_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn insert(&self, draft: AnnouncementDraft) -> Result<(), Unavailable> {
        sqlx::query(
            "INSERT INTO notification.announcement_drafts
                 (announcement_draft_id, created_by, title, body, sport, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(announcement_draft_id_uuid::to_uuid(draft.id()))
        .bind(account_id_uuid::to_uuid(draft.created_by()))
        .bind(draft.title())
        .bind(draft.body())
        .bind(draft.sport().map(sport_value))
        .bind(timestamp(draft.created_at()))
        .bind(timestamp(draft.updated_at()))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn find(
        &self,
        id: AnnouncementDraftId,
    ) -> Result<Option<AnnouncementDraft>, Unavailable> {
        let row: Option<DraftRow> = sqlx::query_as(DRAFT)
            .bind(announcement_draft_id_uuid::to_uuid(id))
            .fetch_optional(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        row.map(draft_from_row).transpose()
    }

    async fn update(&self, draft: AnnouncementDraft) -> Result<bool, Unavailable> {
        // 作った本部のアカウントと作った時刻は書き換えない
        let result = sqlx::query(
            "UPDATE notification.announcement_drafts
             SET title = $2, body = $3, sport = $4, updated_at = $5
             WHERE announcement_draft_id = $1",
        )
        .bind(announcement_draft_id_uuid::to_uuid(draft.id()))
        .bind(draft.title())
        .bind(draft.body())
        .bind(draft.sport().map(sport_value))
        .bind(timestamp(draft.updated_at()))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete(&self, id: AnnouncementDraftId) -> Result<bool, Unavailable> {
        let result = sqlx::query(
            "DELETE FROM notification.announcement_drafts WHERE announcement_draft_id = $1",
        )
        .bind(announcement_draft_id_uuid::to_uuid(id))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(result.rows_affected() > 0)
    }

    async fn find_all(&self) -> Result<Vec<AnnouncementDraft>, Unavailable> {
        let rows: Vec<DraftRow> = sqlx::query_as(ALL_DRAFTS)
            .fetch_all(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        rows.into_iter().map(draft_from_row).collect()
    }
}

fn draft_from_row(row: DraftRow) -> Result<AnnouncementDraft, Unavailable> {
    let (id, created_by, title, body, sport, created_at, updated_at) = row;
    let sport = sport
        .map(|value| {
            sport_from_value(&value)
                .ok_or_else(|| Unavailable::new(format!("未知の sport: {value}")))
        })
        .transpose()?;
    AnnouncementDraft::new(
        announcement_draft_id_uuid::from_uuid(id),
        account_id_uuid::from_uuid(created_by),
        title,
        body,
        sport,
        SystemTime::from(created_at),
    )
    .map(|draft| draft.with_updated_at(SystemTime::from(updated_at)))
    .map_err(Unavailable::new)
}
