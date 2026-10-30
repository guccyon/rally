//! 予約の保存先（Postgres の `scheduling.bookings` と、予約したスロットの `scheduling.slots`）。

use std::time::SystemTime;

use application::{
    Unavailable,
    identity::account_id_uuid,
    scheduling::{
        BookingRepository, ReserveSlotError, UpdateBookingError, booking_id_uuid, slot_id_uuid,
    },
};
use domain::{
    DomainError,
    identity::AccountId,
    scheduling::{Booking, BookingId, Slot, SlotId},
};
use sqlx::{
    FromRow, PgPool, Postgres, Row, Transaction,
    postgres::PgRow,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    pg_slot_repository::{SlotRow, slot_columns, slot_from_row, slot_row_at},
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

/// 予約の列（[`BookingColumns`]）と、続く予約したスロットの列（[`SlotRow`]）。sqlx のタプルの `FromRow` は 16 列
/// までなので、列の位置で読む（[`select_bookings`] の列の順）。
struct BookingRow(BookingColumns, SlotRow);

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
            slot_row_at(row, 7)?,
        ))
    }
}

/// 予約と、予約したスロットを読む SELECT（`$where` は WHERE 句から後ろ）。
macro_rules! select_bookings {
    ($where:literal) => {
        concat!(
            "SELECT b.booking_id, b.student_id, b.is_trial, b.status, b.created_at,
        b.confirmed_at, b.cancelled_at, ",
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

    /// スロットの行をロックして読む。なければ `None`。
    ///
    /// 席の数はロックを取った後の別の文で数える（READ COMMITTED では文ごとにスナップショットを取るので、ロックを
    /// 待っている間に確定した他の予約も数に入る。ロックと同じ文で数えると、待つ前のスナップショットで数えてしまう）。
    async fn lock_slot(
        tx: &mut Transaction<'_, Postgres>,
        slot_id: Uuid,
    ) -> Result<Option<Slot>, Unavailable> {
        let locked: Option<Uuid> = sqlx::query_scalar(
            "SELECT slot_id FROM scheduling.slots WHERE slot_id = $1 FOR UPDATE",
        )
        .bind(slot_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(Unavailable::new)?;
        if locked.is_none() {
            return Ok(None);
        }
        let row: SlotRow = sqlx::query_as(concat!(
            "SELECT ",
            slot_columns!(),
            " FROM scheduling.slots s WHERE s.slot_id = $1"
        ))
        .bind(slot_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(Unavailable::new)?;
        slot_from_row(row).map(Some)
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
        F: FnOnce(&mut Slot, &[Booking]) -> Result<Booking, DomainError> + Send,
    {
        let student = account_id_uuid::to_uuid(student_id);
        let mut tx = self.pool.begin().await.map_err(reserve_unavailable)?;
        let mut slot = Self::lock_slot(&mut tx, slot_id_uuid::to_uuid(slot_id))
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

        sqlx::query(
            "INSERT INTO scheduling.bookings
                 (booking_id, slot_id, student_id, is_trial, status, created_at, confirmed_at, cancelled_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(booking_id_uuid::to_uuid(booking.id()))
        .bind(slot_id_uuid::to_uuid(booking.slot_id()))
        .bind(account_id_uuid::to_uuid(booking.student_id()))
        .bind(booking.is_trial())
        .bind(booking_status_value(booking.status()))
        .bind(timestamp(booking.created_at()))
        .bind(booking.confirmed_at().map(timestamp))
        .bind(booking.cancelled_at().map(timestamp))
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
        F: FnOnce(&mut Booking, &mut Slot) -> Result<(), DomainError> + Send,
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
        let mut slot = Self::lock_slot(&mut tx, slot_id)
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
    );
    Ok((booking, slot))
}
