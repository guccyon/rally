//! コーチのお休みの保存先（Postgres の `scheduling.coach_leaves`・`scheduling.coach_leave_bookings` と、お休みで変える
//! `scheduling.bookings`・`scheduling.slots`）。お休みは予約と枠をまとめて変えるので、[`PgBookingRepository`] が
//! [`CoachLeaveRepository`] も実装する。

use std::{collections::BTreeSet, time::SystemTime};

use application::{
    Unavailable,
    identity::account_id_uuid,
    scheduling::{CoachLeaveRepository, SaveCoachLeaveError, booking_id_uuid, coach_leave_id_uuid},
};
use domain::{
    identity::AccountId,
    scheduling::{
        Booking, BookingId, BookingPeriod, CoachLeave, CoachLeaveBooking, CoachLeaveId,
        CoachLeaveNote, SchedulingError, Slot,
    },
};
use sqlx::{
    PgConnection, Postgres, Transaction,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    pg_booking_repository::{BookingRow, PgBookingRepository, booking_from_row, select_bookings},
    pg_slot_repository::{SlotRow, slot_columns, slot_from_row},
    pg_support::{lock, timestamp},
};

/// `scheduling.coach_leaves` の 1 行（coach_leave_id, coach_id, starts_at, ends_at, note, created_by, created_at,
/// withdrawn_slot_count）。
type CoachLeaveRow = (
    Uuid,
    Uuid,
    OffsetDateTime,
    OffsetDateTime,
    String,
    Uuid,
    OffsetDateTime,
    i32,
);

/// `scheduling.coach_leave_bookings` の 1 行（coach_leave_id, booking_id, student_id, starts_at, ends_at）。
type CoachLeaveBookingRow = (Uuid, Uuid, Uuid, OffsetDateTime, OffsetDateTime);

impl CoachLeaveRepository for PgBookingRepository {
    fn next_coach_leave_id(&self) -> CoachLeaveId {
        coach_leave_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn coach_slots_starting_in(
        &self,
        coach_id: AccountId,
        period: BookingPeriod,
    ) -> Result<(Vec<Slot>, Vec<Booking>), Unavailable> {
        // 枠と予約を同じスナップショットから読む
        let mut tx = self.pool().begin().await.map_err(Unavailable::new)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(Unavailable::new)?;
        let found = calendar_in(&mut tx, coach_id, period).await?;
        tx.commit().await.map_err(Unavailable::new)?;
        Ok(found)
    }

    /// 1 つのトランザクションで、コーチの枠の公開と同じ advisory lock（`scheduling.slots:<コーチ>`）→ 期間の枠の行
    /// （`FOR UPDATE`。ID の順）→ それらの枠への予約の行（`FOR UPDATE`）の順にロックを取ってから読む。予約の作成・確定・
    /// キャンセルは枠の行のロックを先に取るので、確かめた後に入った予約を見落とさない。同じコーチのお休みと枠の公開は
    /// advisory lock で直列になる。
    async fn start_coach_leave<F>(
        &self,
        coach_id: AccountId,
        period: BookingPeriod,
        start: F,
    ) -> Result<CoachLeave, SaveCoachLeaveError>
    where
        F: FnOnce(
                &mut [Slot],
                &mut [Booking],
                &[CoachLeave],
            ) -> Result<CoachLeave, SchedulingError>
            + Send,
    {
        let coach = account_id_uuid::to_uuid(coach_id);
        let mut tx = self.pool().begin().await.map_err(save_unavailable)?;
        lock(&mut tx, &format!("scheduling.slots:{coach}"))
            .await
            .map_err(save_unavailable)?;
        sqlx::query(
            "SELECT slot_id FROM scheduling.slots
             WHERE coach_id = $1 AND starts_at >= $2 AND starts_at < $3
             ORDER BY slot_id FOR UPDATE",
        )
        .bind(coach)
        .bind(timestamp(period.starts_at()))
        .bind(timestamp(period.ends_at()))
        .execute(&mut *tx)
        .await
        .map_err(save_unavailable)?;
        sqlx::query(
            "SELECT b.booking_id FROM scheduling.bookings b
             JOIN scheduling.slots s ON s.slot_id = b.slot_id
             WHERE s.coach_id = $1 AND s.starts_at >= $2 AND s.starts_at < $3
             ORDER BY b.booking_id FOR UPDATE OF b",
        )
        .bind(coach)
        .bind(timestamp(period.starts_at()))
        .bind(timestamp(period.ends_at()))
        .execute(&mut *tx)
        .await
        .map_err(save_unavailable)?;
        // 席の数はロックを取った後の文で数える（lock_slot と同じ理由）
        let (mut slots, mut bookings) = calendar_in(&mut tx, coach_id, period)
            .await
            .map_err(SaveCoachLeaveError::Unavailable)?;
        let leave_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT coach_leave_id FROM scheduling.coach_leaves WHERE coach_id = $1",
        )
        .bind(coach)
        .fetch_all(&mut *tx)
        .await
        .map_err(save_unavailable)?;
        let existing = leaves_by_ids(&mut tx, &leave_ids)
            .await
            .map_err(SaveCoachLeaveError::Unavailable)?;

        let leave =
            start(&mut slots, &mut bookings, &existing).map_err(SaveCoachLeaveError::Rejected)?;

        for booking in &bookings {
            Self::save_booking(&mut tx, booking)
                .await
                .map_err(save_unavailable)?;
        }
        for slot in &slots {
            Self::save_slot_status(&mut tx, slot)
                .await
                .map_err(save_unavailable)?;
        }
        insert_leave(&mut tx, &leave)
            .await
            .map_err(save_unavailable)?;
        tx.commit().await.map_err(save_unavailable)?;
        Ok(leave)
    }

    async fn coach_leaves_of_bookings(
        &self,
        booking_ids: &BTreeSet<BookingId>,
    ) -> Result<Vec<CoachLeave>, Unavailable> {
        let ids: Vec<Uuid> = booking_ids
            .iter()
            .copied()
            .map(booking_id_uuid::to_uuid)
            .collect();
        let mut conn = self.pool().acquire().await.map_err(Unavailable::new)?;
        let leave_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT coach_leave_id FROM scheduling.coach_leave_bookings
             WHERE booking_id = ANY($1)",
        )
        .bind(&ids)
        .fetch_all(&mut *conn)
        .await
        .map_err(Unavailable::new)?;
        leaves_by_ids(&mut conn, &leave_ids).await
    }
}

/// `coach_id` のコーチの、開始が `period` に入る枠と、それらの枠への予約。
async fn calendar_in(
    tx: &mut Transaction<'_, Postgres>,
    coach_id: AccountId,
    period: BookingPeriod,
) -> Result<(Vec<Slot>, Vec<Booking>), Unavailable> {
    let coach = account_id_uuid::to_uuid(coach_id);
    let slot_rows: Vec<SlotRow> = sqlx::query_as(concat!(
        "SELECT ",
        slot_columns!(),
        " FROM scheduling.slots s WHERE s.coach_id = $1 AND s.starts_at >= $2 AND s.starts_at < $3"
    ))
    .bind(coach)
    .bind(timestamp(period.starts_at()))
    .bind(timestamp(period.ends_at()))
    .fetch_all(&mut **tx)
    .await
    .map_err(Unavailable::new)?;
    let booking_rows: Vec<BookingRow> = sqlx::query_as(select_bookings!(
        "WHERE s.coach_id = $1 AND s.starts_at >= $2 AND s.starts_at < $3"
    ))
    .bind(coach)
    .bind(timestamp(period.starts_at()))
    .bind(timestamp(period.ends_at()))
    .fetch_all(&mut **tx)
    .await
    .map_err(Unavailable::new)?;
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

/// `leave_ids` のお休みの記録と、それぞれのキャンセルした予約の写し（枠の開始の早い順）。
async fn leaves_by_ids(
    conn: &mut PgConnection,
    leave_ids: &[Uuid],
) -> Result<Vec<CoachLeave>, Unavailable> {
    if leave_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<CoachLeaveRow> = sqlx::query_as(
        "SELECT coach_leave_id, coach_id, starts_at, ends_at, note, created_by, created_at,
                withdrawn_slot_count
         FROM scheduling.coach_leaves WHERE coach_leave_id = ANY($1) ORDER BY seq",
    )
    .bind(leave_ids)
    .fetch_all(&mut *conn)
    .await
    .map_err(Unavailable::new)?;
    let booking_rows: Vec<CoachLeaveBookingRow> = sqlx::query_as(
        "SELECT coach_leave_id, booking_id, student_id, starts_at, ends_at
         FROM scheduling.coach_leave_bookings WHERE coach_leave_id = ANY($1)
         ORDER BY starts_at, booking_id",
    )
    .bind(leave_ids)
    .fetch_all(&mut *conn)
    .await
    .map_err(Unavailable::new)?;
    rows.into_iter()
        .map(
            |(id, coach_id, starts_at, ends_at, note, created_by, created_at, withdrawn)| {
                let cancelled = booking_rows
                    .iter()
                    .filter(|row| row.0 == id)
                    .map(|&(_, booking_id, student_id, starts_at, ends_at)| {
                        CoachLeaveBooking::new(
                            booking_id_uuid::from_uuid(booking_id),
                            account_id_uuid::from_uuid(student_id),
                            SystemTime::from(starts_at),
                            SystemTime::from(ends_at),
                        )
                    })
                    .collect();
                Ok(CoachLeave::restore(
                    coach_leave_id_uuid::from_uuid(id),
                    account_id_uuid::from_uuid(coach_id),
                    BookingPeriod::new(starts_at.into(), ends_at.into())
                        .map_err(Unavailable::new)?,
                    CoachLeaveNote::new(note).map_err(Unavailable::new)?,
                    account_id_uuid::from_uuid(created_by),
                    created_at.into(),
                    cancelled,
                    u32::try_from(withdrawn).map_err(Unavailable::new)?,
                ))
            },
        )
        .collect()
}

/// お休みの記録と、キャンセルした予約の写しを書く。
async fn insert_leave(
    tx: &mut Transaction<'_, Postgres>,
    leave: &CoachLeave,
) -> Result<(), sqlx::Error> {
    let id = coach_leave_id_uuid::to_uuid(leave.id());
    sqlx::query(
        "INSERT INTO scheduling.coach_leaves
           (coach_leave_id, coach_id, starts_at, ends_at, note, created_by, created_at, withdrawn_slot_count)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id)
    .bind(account_id_uuid::to_uuid(leave.coach_id()))
    .bind(timestamp(leave.period().starts_at()))
    .bind(timestamp(leave.period().ends_at()))
    .bind(leave.note().as_str())
    .bind(account_id_uuid::to_uuid(leave.created_by()))
    .bind(timestamp(leave.created_at()))
    .bind(i32::try_from(leave.withdrawn_slot_count()).unwrap_or(i32::MAX))
    .execute(&mut **tx)
    .await?;
    for booking in leave.cancelled_bookings() {
        sqlx::query(
            "INSERT INTO scheduling.coach_leave_bookings
               (booking_id, coach_leave_id, student_id, starts_at, ends_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(booking_id_uuid::to_uuid(booking.booking_id()))
        .bind(id)
        .bind(account_id_uuid::to_uuid(booking.student_id()))
        .bind(timestamp(booking.starts_at()))
        .bind(timestamp(booking.ends_at()))
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn save_unavailable(error: sqlx::Error) -> SaveCoachLeaveError {
    SaveCoachLeaveError::Unavailable(Unavailable::new(error))
}
