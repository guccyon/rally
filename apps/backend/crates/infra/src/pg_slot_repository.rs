//! スロットの保存先（Postgres の `scheduling.slots`）。

use application::{
    Unavailable,
    identity::account_id_uuid,
    scheduling::{AddToCalendarError, SlotCriteria, SlotRepository, slot_id_uuid},
};
use domain::{
    DomainError,
    scheduling::{CoachCalendar, Slot, SlotId, SlotStatus},
};
use sqlx::{
    PgPool, Postgres, Transaction,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    identity_values::{sport_from_value, sport_value},
    pg_support::{lock, timestamp},
    scheduling_values::{slot_status_from_value, slot_status_value},
};

/// [`SlotRepository`] の Postgres 実装。
///
/// 同じコーチの枠が重ならないことは domain の [`CoachCalendar`] だけで判定し、DB の排他制約は
/// 置かない（ADR 0004 の 4）。並行な公開が互いを見落とさないよう、[`SlotRepository::add_to_calendar`]
/// はトランザクションの中でコーチごとの advisory lock を取ってから、カレンダーの読み取りと保存を行う。
#[derive(Debug, Clone)]
pub struct PgSlotRepository {
    pool: PgPool,
}

/// `scheduling.slots` の 1 行（slot_id, coach_id, sport, starts_at, ends_at, status）。
pub(crate) type SlotRow = (Uuid, Uuid, String, OffsetDateTime, OffsetDateTime, String);

impl PgSlotRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn lock_coach(
        tx: &mut Transaction<'_, Postgres>,
        coach_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        // トランザクションの終わりに解放される。キーはコーチ ID から作る
        lock(tx, &format!("scheduling.slots:{coach_id}")).await
    }
}

impl SlotRepository for PgSlotRepository {
    fn next_id(&self) -> SlotId {
        slot_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn add_to_calendar<F>(&self, slot: Slot, add: F) -> Result<(), AddToCalendarError>
    where
        F: FnOnce(&mut CoachCalendar, Slot) -> Result<(), DomainError> + Send,
    {
        let coach_id = account_id_uuid::to_uuid(slot.coach_id());
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        Self::lock_coach(&mut tx, coach_id)
            .await
            .map_err(unavailable)?;

        // slot の期間に重なる同じコーチの枠（状態を問わない。時間を占めるかは domain が決める）
        let rows: Vec<SlotRow> = sqlx::query_as(
            "SELECT slot_id, coach_id, sport, starts_at, ends_at, status FROM scheduling.slots
             WHERE coach_id = $1 AND starts_at < $3 AND ends_at > $2",
        )
        .bind(coach_id)
        .bind(timestamp(slot.starts_at()))
        .bind(timestamp(slot.ends_at()))
        .fetch_all(&mut *tx)
        .await
        .map_err(unavailable)?;
        let existing = rows
            .into_iter()
            .map(slot_from_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(AddToCalendarError::Unavailable)?;
        let mut calendar = CoachCalendar::new(slot.coach_id(), existing)
            .map_err(|e| AddToCalendarError::Unavailable(Unavailable::new(e)))?;
        add(&mut calendar, slot.clone()).map_err(AddToCalendarError::Rejected)?;

        sqlx::query(
            "INSERT INTO scheduling.slots (slot_id, coach_id, sport, starts_at, ends_at, status)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(slot_id_uuid::to_uuid(slot.id()))
        .bind(coach_id)
        .bind(sport_value(slot.sport()))
        .bind(timestamp(slot.starts_at()))
        .bind(timestamp(slot.ends_at()))
        .bind(slot_status_value(slot.status()))
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)
    }

    async fn search(&self, criteria: &SlotCriteria) -> Result<Vec<Slot>, Unavailable> {
        let (after_starts_at, after_slot_id) = criteria
            .after
            .map(|c| (timestamp(c.starts_at), slot_id_uuid::to_uuid(c.slot_id)))
            .unzip();
        let limit = i64::try_from(criteria.limit).unwrap_or(i64::MAX);
        let rows: Vec<SlotRow> = sqlx::query_as(
            "SELECT slot_id, coach_id, sport, starts_at, ends_at, status FROM scheduling.slots
             WHERE status = $1
               AND sport = $2
               AND ($3::uuid IS NULL OR coach_id = $3)
               AND starts_at >= $4
               AND ($5::timestamptz IS NULL OR starts_at < $5)
               AND ($6::timestamptz IS NULL OR (starts_at, slot_id) > ($6, $7::uuid))
             ORDER BY starts_at, slot_id
             LIMIT $8",
        )
        .bind(slot_status_value(SlotStatus::Open))
        .bind(sport_value(criteria.sport))
        .bind(criteria.coach_id.map(account_id_uuid::to_uuid))
        .bind(timestamp(criteria.starts_after))
        .bind(criteria.starts_before.map(timestamp))
        .bind(after_starts_at)
        .bind(after_slot_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        rows.into_iter().map(slot_from_row).collect()
    }
}

fn unavailable(error: sqlx::Error) -> AddToCalendarError {
    AddToCalendarError::Unavailable(Unavailable::new(error))
}

pub(crate) fn slot_from_row(
    (slot_id, coach_id, sport, starts_at, ends_at, status): SlotRow,
) -> Result<Slot, Unavailable> {
    let sport = sport_from_value(&sport)
        .ok_or_else(|| Unavailable::new(format!("未知の sport: {sport}")))?;
    let status = slot_status_from_value(&status)
        .ok_or_else(|| Unavailable::new(format!("未知の status: {status}")))?;
    Slot::restore(
        slot_id_uuid::from_uuid(slot_id),
        account_id_uuid::from_uuid(coach_id),
        sport,
        starts_at.into(),
        ends_at.into(),
        status,
    )
    .map_err(Unavailable::new)
}
