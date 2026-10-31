//! スロットの保存先（Postgres の `scheduling.slots`）。

use application::{
    Unavailable,
    catalog::lesson_menu_id_uuid,
    identity::account_id_uuid,
    scheduling::{AddToCalendarError, SlotCriteria, SlotRepository, slot_id_uuid},
};
use domain::{
    identity::AccountId,
    scheduling::{
        CoachCalendar, SchedulingError, Slot, SlotCapacity, SlotId, SlotSeats, SlotStatus,
    },
};
use sqlx::{
    PgPool, Postgres, Row, Transaction,
    postgres::PgRow,
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

/// `scheduling.slots` の 1 行（slot_id, coach_id, sport, starts_at, ends_at, status, capacity, lesson_menu_id）と、
/// その枠の未確定・確定の予約の数（[`slot_columns`]）。
pub(crate) type SlotRow = (
    Uuid,
    Uuid,
    String,
    OffsetDateTime,
    OffsetDateTime,
    String,
    i16,
    Option<Uuid>,
    i64,
    i64,
);

/// [`SlotRow`] を読む SELECT の列（文字列リテラル）。スロットの別名は `s`。有効な予約の数（domain の [`SlotSeats`]）は
/// 列に持たず、`scheduling.bookings` から状態ごとに数える。状態の値は [`crate::scheduling_values::booking_status_value`] と同じ（テストで確かめる）。
macro_rules! slot_columns {
    () => {
        "s.slot_id, s.coach_id, s.sport, s.starts_at, s.ends_at, s.status, s.capacity, s.lesson_menu_id,
        (SELECT count(*) FROM scheduling.bookings sb WHERE sb.slot_id = s.slot_id AND sb.status = 'pending'),
        (SELECT count(*) FROM scheduling.bookings sb WHERE sb.slot_id = s.slot_id AND sb.status = 'confirmed')"
    };
}
pub(crate) use slot_columns;

/// `row` の `offset` 列目から [`SlotRow`] を読む（[`slot_columns`] を他の列の後ろに並べた SELECT 用）。
pub(crate) fn slot_row_at(row: &PgRow, offset: usize) -> Result<SlotRow, sqlx::Error> {
    Ok((
        row.try_get(offset)?,
        row.try_get(offset + 1)?,
        row.try_get(offset + 2)?,
        row.try_get(offset + 3)?,
        row.try_get(offset + 4)?,
        row.try_get(offset + 5)?,
        row.try_get(offset + 6)?,
        row.try_get(offset + 7)?,
        row.try_get(offset + 8)?,
        row.try_get(offset + 9)?,
    ))
}

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
        F: FnOnce(&mut CoachCalendar, Slot) -> Result<(), SchedulingError> + Send,
    {
        let coach_id = account_id_uuid::to_uuid(slot.coach_id());
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        Self::lock_coach(&mut tx, coach_id)
            .await
            .map_err(unavailable)?;

        // slot の期間に重なる同じコーチの枠（状態を問わない。時間を占めるかは domain が決める）
        let rows: Vec<SlotRow> = sqlx::query_as(concat!(
            "SELECT ",
            slot_columns!(),
            " FROM scheduling.slots s
             WHERE s.coach_id = $1 AND s.starts_at < $3 AND s.ends_at > $2"
        ))
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
            "INSERT INTO scheduling.slots
                 (slot_id, coach_id, sport, starts_at, ends_at, status, capacity, lesson_menu_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(slot_id_uuid::to_uuid(slot.id()))
        .bind(coach_id)
        .bind(sport_value(slot.sport()))
        .bind(timestamp(slot.starts_at()))
        .bind(timestamp(slot.ends_at()))
        .bind(slot_status_value(slot.status()))
        .bind(i16::from(slot.capacity().value()))
        .bind(slot.lesson_menu_id().map(lesson_menu_id_uuid::to_uuid))
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
        let rows: Vec<SlotRow> = sqlx::query_as(concat!(
            "SELECT ",
            slot_columns!(),
            " FROM scheduling.slots s
             WHERE s.status = $1
               AND s.sport = $2
               AND ($3::uuid IS NULL OR s.coach_id = $3)
               AND s.starts_at >= $4
               AND ($5::timestamptz IS NULL OR s.starts_at < $5)
               AND ($6::timestamptz IS NULL OR (s.starts_at, s.slot_id) > ($6, $7::uuid))
             ORDER BY s.starts_at, s.slot_id
             LIMIT $8"
        ))
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

    async fn slots_of_coach(&self, coach_id: AccountId) -> Result<Vec<Slot>, Unavailable> {
        let rows: Vec<SlotRow> = sqlx::query_as(concat!(
            "SELECT ",
            slot_columns!(),
            " FROM scheduling.slots s WHERE s.coach_id = $1"
        ))
        .bind(account_id_uuid::to_uuid(coach_id))
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
    (
        slot_id,
        coach_id,
        sport,
        starts_at,
        ends_at,
        status,
        capacity,
        lesson_menu_id,
        pending,
        confirmed,
    ): SlotRow,
) -> Result<Slot, Unavailable> {
    let sport = sport_from_value(&sport)
        .ok_or_else(|| Unavailable::new(format!("未知の sport: {sport}")))?;
    let status = slot_status_from_value(&status)
        .ok_or_else(|| Unavailable::new(format!("未知の status: {status}")))?;
    let capacity = SlotCapacity::new(capacity.into()).map_err(Unavailable::new)?;
    let count = |n: i64| u32::try_from(n).map_err(Unavailable::new);
    let seats = SlotSeats::restore(capacity, count(pending)?, count(confirmed)?)
        .map_err(Unavailable::new)?;
    Slot::restore(
        slot_id_uuid::from_uuid(slot_id),
        account_id_uuid::from_uuid(coach_id),
        sport,
        starts_at.into(),
        ends_at.into(),
        status,
        seats,
    )
    .map(|slot| slot.with_restored_lesson_menu(lesson_menu_id.map(lesson_menu_id_uuid::from_uuid)))
    .map_err(Unavailable::new)
}

#[cfg(test)]
mod tests {
    use domain::scheduling::BookingStatus;

    use crate::scheduling_values::booking_status_value;

    #[test]
    fn slot_columns_count_bookings_by_the_stored_status_values() {
        for status in [BookingStatus::Pending, BookingStatus::Confirmed] {
            let value = format!("sb.status = '{}'", booking_status_value(status));
            assert!(slot_columns!().contains(&value), "{value}");
        }
    }
}
