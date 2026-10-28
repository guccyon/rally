//! 通話セッションの保存先（Postgres の `live_lesson.sessions` と、結び付いた予約の `live_lesson.session_bookings`）。

use application::{
    Unavailable,
    identity::account_id_uuid,
    live_lesson::{
        InsertSessionError, LiveLessonSessionRepository, UpdateSessionError, session_id_uuid,
    },
    scheduling::booking_id_uuid,
};
use domain::{
    DomainError,
    live_lesson::{ConnectionQuality, LiveLessonSession, SessionBooking, SessionId},
    scheduling::BookingId,
};
use sqlx::{
    PgConnection, PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::pg_support::{is_unique_violation, timestamp};

/// [`LiveLessonSessionRepository`] の Postgres 実装。
///
/// 「同じ予約のセッションは 1 つ」は DB の `sessions_booking_id_key`（開始した予約）と
/// `session_bookings_booking_id_key`（結び付いた予約）で守る。セッションと結び付いた予約は 1 つのトランザクションで
/// 保存する。`session_bookings` に行のないセッション（D41 より前の版が始めたもの）は、開始した予約だけが結び付いた
/// ものとして読む。終了は行のロック（`FOR UPDATE`）の中で読み取りから保存までを行う。
#[derive(Debug, Clone)]
pub struct PgLiveLessonSessionRepository {
    pool: PgPool,
}

/// `live_lesson.sessions` の 1 行（session_id, booking_id, student_id, coach_id, started_at, ended_at,
/// packet_loss_rate, jitter_ms, round_trip_ms, disconnect_count）。
type SessionRow = (
    Uuid,
    Uuid,
    Uuid,
    Uuid,
    OffsetDateTime,
    Option<OffsetDateTime>,
    Option<f64>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

/// セッションを読む SELECT（`$where` は WHERE 句から後ろ）。
macro_rules! select_sessions {
    ($where:literal) => {
        concat!(
            "SELECT session_id, booking_id, student_id, coach_id, started_at, ended_at,
                    packet_loss_rate, jitter_ms, round_trip_ms, disconnect_count
             FROM live_lesson.sessions ",
            $where
        )
    };
}

impl PgLiveLessonSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// `row` のセッションを、結び付いた予約を読んで組み立てる。
async fn session_with_bookings(
    conn: &mut PgConnection,
    row: SessionRow,
) -> Result<LiveLessonSession, Unavailable> {
    let bookings: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT booking_id, student_id FROM live_lesson.session_bookings
         WHERE session_id = $1 ORDER BY position",
    )
    .bind(row.0)
    .fetch_all(conn)
    .await
    .map_err(Unavailable::new)?;
    let bookings = bookings
        .into_iter()
        .map(|(booking_id, student_id)| {
            SessionBooking::new(
                booking_id_uuid::from_uuid(booking_id),
                account_id_uuid::from_uuid(student_id),
            )
        })
        .collect();
    session_from_row(row, bookings)
}

impl LiveLessonSessionRepository for PgLiveLessonSessionRepository {
    fn next_id(&self) -> SessionId {
        session_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn find(&self, session_id: SessionId) -> Result<Option<LiveLessonSession>, Unavailable> {
        let mut conn = self.pool.acquire().await.map_err(Unavailable::new)?;
        let row: Option<SessionRow> = sqlx::query_as(select_sessions!("WHERE session_id = $1"))
            .bind(session_id_uuid::to_uuid(session_id))
            .fetch_optional(&mut *conn)
            .await
            .map_err(Unavailable::new)?;
        match row {
            Some(row) => session_with_bookings(&mut conn, row).await.map(Some),
            None => Ok(None),
        }
    }

    async fn find_by_booking(
        &self,
        booking_id: BookingId,
    ) -> Result<Option<LiveLessonSession>, Unavailable> {
        let mut conn = self.pool.acquire().await.map_err(Unavailable::new)?;
        let row: Option<SessionRow> = sqlx::query_as(select_sessions!(
            "WHERE booking_id = $1
                OR session_id IN (SELECT session_id FROM live_lesson.session_bookings WHERE booking_id = $1)"
        ))
        .bind(booking_id_uuid::to_uuid(booking_id))
        .fetch_optional(&mut *conn)
        .await
        .map_err(Unavailable::new)?;
        match row {
            Some(row) => session_with_bookings(&mut conn, row).await.map(Some),
            None => Ok(None),
        }
    }

    async fn insert(&self, session: LiveLessonSession) -> Result<(), InsertSessionError> {
        let already_has_session = |error: sqlx::Error| {
            if is_unique_violation(&error, "sessions_booking_id_key")
                || is_unique_violation(&error, "session_bookings_booking_id_key")
            {
                InsertSessionError::BookingAlreadyHasSession
            } else {
                InsertSessionError::Unavailable(Unavailable::new(error))
            }
        };
        let mut tx = self.pool.begin().await.map_err(already_has_session)?;
        sqlx::query(
            "INSERT INTO live_lesson.sessions (session_id, booking_id, student_id, coach_id, started_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(session_id_uuid::to_uuid(session.id()))
        .bind(booking_id_uuid::to_uuid(session.booking_id()))
        .bind(account_id_uuid::to_uuid(session.student_id()))
        .bind(account_id_uuid::to_uuid(session.coach_id()))
        .bind(timestamp(session.started_at()))
        .execute(&mut *tx)
        .await
        .map_err(already_has_session)?;
        for (position, booking) in (0_i16..).zip(session.bookings()) {
            sqlx::query(
                "INSERT INTO live_lesson.session_bookings (session_id, booking_id, student_id, position)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(session_id_uuid::to_uuid(session.id()))
            .bind(booking_id_uuid::to_uuid(booking.booking_id()))
            .bind(account_id_uuid::to_uuid(booking.student_id()))
            .bind(position)
            .execute(&mut *tx)
            .await
            .map_err(already_has_session)?;
        }
        tx.commit().await.map_err(already_has_session)?;
        Ok(())
    }

    async fn update<F>(
        &self,
        session_id: SessionId,
        update: F,
    ) -> Result<LiveLessonSession, UpdateSessionError>
    where
        F: FnOnce(&mut LiveLessonSession) -> Result<(), DomainError> + Send,
    {
        let unavailable = |e: sqlx::Error| UpdateSessionError::Unavailable(Unavailable::new(e));
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let row: Option<SessionRow> =
            sqlx::query_as(select_sessions!("WHERE session_id = $1 FOR UPDATE"))
                .bind(session_id_uuid::to_uuid(session_id))
                .fetch_optional(&mut *tx)
                .await
                .map_err(unavailable)?;
        let row = row.ok_or(UpdateSessionError::NotFound)?;
        let mut session = session_with_bookings(&mut tx, row)
            .await
            .map_err(UpdateSessionError::Unavailable)?;
        update(&mut session).map_err(UpdateSessionError::Rejected)?;

        let quality = session.quality();
        sqlx::query(
            "UPDATE live_lesson.sessions
             SET ended_at = $2, packet_loss_rate = $3, jitter_ms = $4, round_trip_ms = $5,
                 disconnect_count = $6
             WHERE session_id = $1",
        )
        .bind(session_id_uuid::to_uuid(session.id()))
        .bind(session.ended_at().map(timestamp))
        .bind(quality.map(|q| q.packet_loss_rate()))
        .bind(quality.map(|q| to_i32(q.jitter_ms())))
        .bind(quality.map(|q| to_i32(q.round_trip_ms())))
        .bind(quality.map(|q| to_i32(q.disconnect_count())))
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(session)
    }
}

fn to_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn session_from_row(
    (
        session_id,
        booking_id,
        student_id,
        coach_id,
        started_at,
        ended_at,
        packet_loss_rate,
        jitter_ms,
        round_trip_ms,
        disconnect_count,
    ): SessionRow,
    bookings: Vec<SessionBooking>,
) -> Result<LiveLessonSession, Unavailable> {
    let ended = match (
        ended_at,
        packet_loss_rate,
        jitter_ms,
        round_trip_ms,
        disconnect_count,
    ) {
        (None, ..) => None,
        (Some(ended_at), Some(loss), Some(jitter), Some(round_trip), Some(disconnects)) => {
            let to_u32 = |v: i32| u32::try_from(v).map_err(Unavailable::new);
            let quality = ConnectionQuality::new(
                loss,
                to_u32(jitter)?,
                to_u32(round_trip)?,
                to_u32(disconnects)?,
            )
            .map_err(Unavailable::new)?;
            Some((ended_at.into(), quality))
        }
        _ => return Err(Unavailable::new("終了したセッションに接続品質がない")),
    };
    LiveLessonSession::restore(
        session_id_uuid::from_uuid(session_id),
        booking_id_uuid::from_uuid(booking_id),
        account_id_uuid::from_uuid(student_id),
        account_id_uuid::from_uuid(coach_id),
        bookings,
        started_at.into(),
        ended,
    )
    .map_err(Unavailable::new)
}
