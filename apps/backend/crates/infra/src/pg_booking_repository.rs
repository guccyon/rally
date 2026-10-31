//! 予約の保存先（Postgres の `scheduling.bookings` と、予約したスロットの `scheduling.slots`）。

use std::{
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};

use application::{
    Unavailable,
    catalog::lesson_menu_id_uuid,
    identity::account_id_uuid,
    scheduling::{
        BookingRepository, ReserveSlotError, UpdateBookingError, booking_id_uuid, slot_id_uuid,
    },
};
use domain::{
    catalog::{LessonDuration, LessonMenuName},
    identity::AccountId,
    scheduling::{Booking, BookingId, BookingLessonMenu, SchedulingError, Slot, SlotId},
};
use sqlx::{
    FromRow, PgPool, Postgres, Row, Transaction,
    postgres::PgRow,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    pg_slot_repository::{SlotRow, lock_slot, slot_columns, slot_from_row, slot_row_at},
    pg_support::{lock, timestamp},
    scheduling_values::{booking_status_from_value, booking_status_value, slot_status_value},
};

/// [`BookingRepository`] の Postgres 実装。
///
/// 不変条件の判定は domain の [`Booking`] だけが行う。読み取りから保存までは 1 つのトランザクションで、
/// スロットの行（`FOR UPDATE`）→ 生徒の advisory lock（予約の作成のみ。体験予約の判定のため）→ 予約の行の順に
/// ロックを取る。スロットの席の数（有効な予約の数）はロックを取った後の文で予約の行から数えるので、並行な予約が
/// 互いを見落として定員を超えることはない。同じ生徒の同じ枠への二重予約は DB の
/// `bookings_one_active_per_slot_student` でも防ぐ。
#[derive(Debug, Clone)]
pub struct PgBookingRepository {
    pool: PgPool,
}

/// 予約の列（booking_id, student_id, is_trial, status, created_at, confirmed_at, cancelled_at）。
type BookingColumns = (
    Uuid,
    Uuid,
    bool,
    String,
    OffsetDateTime,
    Option<OffsetDateTime>,
    Option<OffsetDateTime>,
);

/// 予約した時点のレッスンメニューの写しの列（lesson_menu_id, lesson_menu_name, lesson_menu_duration_minutes）。
/// 写しのない予約はどれも NULL（DB の `bookings_lesson_menu_shape`）。
type LessonMenuColumns = (Option<Uuid>, Option<String>, Option<i16>);

/// 予約の列（[`BookingColumns`]）と、レッスンメニューの写しの列（[`LessonMenuColumns`]）と、続く予約したスロットの列
/// （[`SlotRow`]）。sqlx のタプルの `FromRow` は 16 列までなので、列の位置で読む（[`select_bookings`] の列の順）。
struct BookingRow(BookingColumns, LessonMenuColumns, SlotRow);

impl<'r> FromRow<'r, PgRow> for BookingRow {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self(
            (
                row.try_get(0)?,
                row.try_get(1)?,
                row.try_get(2)?,
                row.try_get(3)?,
                row.try_get(4)?,
                row.try_get(5)?,
                row.try_get(6)?,
            ),
            (row.try_get(7)?, row.try_get(8)?, row.try_get(9)?),
            slot_row_at(row, 10)?,
        ))
    }
}

/// 予約と、予約したスロットを読む SELECT（`$where` は WHERE 句から後ろ）。
macro_rules! select_bookings {
    ($where:literal) => {
        concat!(
            "SELECT b.booking_id, b.student_id, b.is_trial, b.status, b.created_at,
        b.confirmed_at, b.cancelled_at, b.lesson_menu_id, b.lesson_menu_name,
        b.lesson_menu_duration_minutes, ",
            slot_columns!(),
            " FROM scheduling.bookings b JOIN scheduling.slots s ON s.slot_id = b.slot_id ",
            $where
        )
    };
}

impl PgBookingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn save(
        tx: &mut Transaction<'_, Postgres>,
        booking: &Booking,
        slot: &Slot,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE scheduling.bookings SET status = $2, confirmed_at = $3, cancelled_at = $4
             WHERE booking_id = $1",
        )
        .bind(booking_id_uuid::to_uuid(booking.id()))
        .bind(booking_status_value(booking.status()))
        .bind(booking.confirmed_at().map(timestamp))
        .bind(booking.cancelled_at().map(timestamp))
        .execute(&mut **tx)
        .await?;
        sqlx::query("UPDATE scheduling.slots SET status = $2 WHERE slot_id = $1")
            .bind(slot_id_uuid::to_uuid(slot.id()))
            .bind(slot_status_value(slot.status()))
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}

impl BookingRepository for PgBookingRepository {
    fn next_id(&self) -> BookingId {
        booking_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn find_slot(&self, slot_id: SlotId) -> Result<Option<Slot>, Unavailable> {
        let row: Option<SlotRow> = sqlx::query_as(concat!(
            "SELECT ",
            slot_columns!(),
            " FROM scheduling.slots s WHERE s.slot_id = $1"
        ))
        .bind(slot_id_uuid::to_uuid(slot_id))
        .fetch_optional(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        row.map(slot_from_row).transpose()
    }

    async fn find(&self, booking_id: BookingId) -> Result<Option<Booking>, Unavailable> {
        let row: Option<BookingRow> = sqlx::query_as(select_bookings!("WHERE b.booking_id = $1"))
            .bind(booking_id_uuid::to_uuid(booking_id))
            .fetch_optional(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        row.map(booking_from_row).transpose()
    }

    async fn reserve<F>(
        &self,
        slot_id: SlotId,
        student_id: AccountId,
        reserve: F,
    ) -> Result<Booking, ReserveSlotError>
    where
        F: FnOnce(&mut Slot, &[Booking]) -> Result<Booking, SchedulingError> + Send,
    {
        let student = account_id_uuid::to_uuid(student_id);
        let mut tx = self.pool.begin().await.map_err(reserve_unavailable)?;
        let mut slot = lock_slot(&mut tx, slot_id_uuid::to_uuid(slot_id))
            .await
            .map_err(ReserveSlotError::Unavailable)?
            .ok_or(ReserveSlotError::SlotNotFound)?;
        lock(&mut tx, &format!("scheduling.bookings:{student}"))
            .await
            .map_err(reserve_unavailable)?;

        let rows: Vec<BookingRow> = sqlx::query_as(select_bookings!(
            "WHERE b.slot_id = $1 OR b.student_id = $2"
        ))
        .bind(slot_id_uuid::to_uuid(slot_id))
        .bind(student)
        .fetch_all(&mut *tx)
        .await
        .map_err(reserve_unavailable)?;
        let existing = rows
            .into_iter()
            .map(booking_from_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ReserveSlotError::Unavailable)?;
        // 押さえた席（slot の数）は保存しない。数は予約の行から数える
        let booking = reserve(&mut slot, &existing).map_err(ReserveSlotError::Rejected)?;

        let lesson_menu = booking.lesson_menu();
        sqlx::query(
            "INSERT INTO scheduling.bookings
                 (booking_id, slot_id, student_id, is_trial, status, created_at, confirmed_at, cancelled_at,
                  lesson_menu_id, lesson_menu_name, lesson_menu_duration_minutes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(booking_id_uuid::to_uuid(booking.id()))
        .bind(slot_id_uuid::to_uuid(booking.slot_id()))
        .bind(account_id_uuid::to_uuid(booking.student_id()))
        .bind(booking.is_trial())
        .bind(booking_status_value(booking.status()))
        .bind(timestamp(booking.created_at()))
        .bind(booking.confirmed_at().map(timestamp))
        .bind(booking.cancelled_at().map(timestamp))
        .bind(lesson_menu.map(|menu| lesson_menu_id_uuid::to_uuid(menu.id())))
        .bind(lesson_menu.map(|menu| menu.name().as_str()))
        .bind(
            lesson_menu
                .and_then(BookingLessonMenu::duration)
                .map(|duration| i16::from(duration.minutes())),
        )
        .execute(&mut *tx)
        .await
        .map_err(reserve_unavailable)?;
        tx.commit().await.map_err(reserve_unavailable)?;
        Ok(booking)
    }

    async fn update<F>(
        &self,
        booking_id: BookingId,
        update: F,
    ) -> Result<Booking, UpdateBookingError>
    where
        F: FnOnce(&mut Booking, &mut Slot) -> Result<(), SchedulingError> + Send,
    {
        let booking_id = booking_id_uuid::to_uuid(booking_id);
        let mut tx = self.pool.begin().await.map_err(update_unavailable)?;
        // 予約のスロットは変わらないので、先にスロットを知ってからスロット → 予約の順にロックする
        let slot_id: Uuid =
            sqlx::query_scalar("SELECT slot_id FROM scheduling.bookings WHERE booking_id = $1")
                .bind(booking_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(update_unavailable)?
                .ok_or(UpdateBookingError::BookingNotFound)?;
        let mut slot = lock_slot(&mut tx, slot_id)
            .await
            .map_err(UpdateBookingError::Unavailable)?
            .ok_or_else(|| {
                UpdateBookingError::Unavailable(Unavailable::new(
                    "予約のスロットが保存されていない",
                ))
            })?;
        let row: BookingRow =
            sqlx::query_as(select_bookings!("WHERE b.booking_id = $1 FOR UPDATE OF b"))
                .bind(booking_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(update_unavailable)?;
        let mut booking = booking_from_row(row).map_err(UpdateBookingError::Unavailable)?;

        update(&mut booking, &mut slot).map_err(UpdateBookingError::Rejected)?;
        Self::save(&mut tx, &booking, &slot)
            .await
            .map_err(update_unavailable)?;
        tx.commit().await.map_err(update_unavailable)?;
        Ok(booking)
    }

    async fn slots_starting_between(
        &self,
        from: SystemTime,
        until: SystemTime,
    ) -> Result<(Vec<Slot>, Vec<Booking>), Unavailable> {
        // 枠と予約を同じスナップショットから読む
        let mut tx = self.pool.begin().await.map_err(Unavailable::new)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(Unavailable::new)?;
        let slot_rows: Vec<SlotRow> = sqlx::query_as(concat!(
            "SELECT ",
            slot_columns!(),
            " FROM scheduling.slots s WHERE s.starts_at >= $1 AND s.starts_at < $2"
        ))
        .bind(timestamp(from))
        .bind(timestamp(until))
        .fetch_all(&mut *tx)
        .await
        .map_err(Unavailable::new)?;
        let booking_rows: Vec<BookingRow> = sqlx::query_as(select_bookings!(
            "WHERE s.starts_at >= $1 AND s.starts_at < $2"
        ))
        .bind(timestamp(from))
        .bind(timestamp(until))
        .fetch_all(&mut *tx)
        .await
        .map_err(Unavailable::new)?;
        tx.commit().await.map_err(Unavailable::new)?;
        let slots = slot_rows
            .into_iter()
            .map(slot_from_row)
            .collect::<Result<_, _>>()?;
        let bookings = booking_rows
            .into_iter()
            .map(booking_from_row)
            .collect::<Result<_, _>>()?;
        Ok((slots, bookings))
    }

    async fn bookings_of_student(
        &self,
        student_id: AccountId,
    ) -> Result<Vec<(Booking, Slot)>, Unavailable> {
        let rows: Vec<BookingRow> = sqlx::query_as(select_bookings!("WHERE b.student_id = $1"))
            .bind(account_id_uuid::to_uuid(student_id))
            .fetch_all(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        rows.into_iter().map(booking_and_slot_from_row).collect()
    }

    async fn last_booked_at_of_students(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, SystemTime>, Unavailable> {
        let ids: Vec<Uuid> = student_ids
            .iter()
            .copied()
            .map(account_id_uuid::to_uuid)
            .collect();
        let rows: Vec<(Uuid, OffsetDateTime)> = sqlx::query_as(
            "SELECT student_id, max(created_at) FROM scheduling.bookings
             WHERE student_id = ANY($1) GROUP BY student_id",
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(rows
            .into_iter()
            .map(|(student_id, at)| (account_id_uuid::from_uuid(student_id), at.into()))
            .collect())
    }

    async fn bookings_of_coach(
        &self,
        coach_id: AccountId,
    ) -> Result<Vec<(Booking, Slot)>, Unavailable> {
        let rows: Vec<BookingRow> = sqlx::query_as(select_bookings!("WHERE s.coach_id = $1"))
            .bind(account_id_uuid::to_uuid(coach_id))
            .fetch_all(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        rows.into_iter().map(booking_and_slot_from_row).collect()
    }
}

fn reserve_unavailable(error: sqlx::Error) -> ReserveSlotError {
    ReserveSlotError::Unavailable(Unavailable::new(error))
}

fn update_unavailable(error: sqlx::Error) -> UpdateBookingError {
    UpdateBookingError::Unavailable(Unavailable::new(error))
}

fn booking_from_row(row: BookingRow) -> Result<Booking, Unavailable> {
    booking_and_slot_from_row(row).map(|(booking, _)| booking)
}

/// 予約と、予約したスロット。
fn booking_and_slot_from_row(
    BookingRow(
        (booking_id, student_id, is_trial, status, created_at, confirmed_at, cancelled_at),
        lesson_menu,
        slot,
    ): BookingRow,
) -> Result<(Booking, Slot), Unavailable> {
    let slot = slot_from_row(slot)?;
    let status = booking_status_from_value(&status)
        .ok_or_else(|| Unavailable::new(format!("未知の status: {status}")))?;
    let booking = Booking::restore(
        booking_id_uuid::from_uuid(booking_id),
        account_id_uuid::from_uuid(student_id),
        &slot,
        is_trial,
        status,
        created_at.into(),
        confirmed_at.map(Into::into),
        cancelled_at.map(Into::into),
    )
    .with_restored_lesson_menu(lesson_menu_from_columns(lesson_menu)?);
    Ok((booking, slot))
}

/// 予約した時点のレッスンメニューの写し。写しのない予約は `None`。
fn lesson_menu_from_columns(
    (lesson_menu_id, name, duration_minutes): LessonMenuColumns,
) -> Result<Option<BookingLessonMenu>, Unavailable> {
    let (Some(lesson_menu_id), Some(name)) = (lesson_menu_id, name) else {
        return Ok(None);
    };
    let duration = duration_minutes
        .map(|minutes| LessonDuration::from_minutes(i64::from(minutes)))
        .transpose()
        .map_err(Unavailable::new)?;
    Ok(Some(BookingLessonMenu::restore(
        lesson_menu_id_uuid::from_uuid(lesson_menu_id),
        LessonMenuName::new(name).map_err(Unavailable::new)?,
        duration,
    )))
}
