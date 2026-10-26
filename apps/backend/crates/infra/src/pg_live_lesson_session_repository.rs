//! 通話セッションの保存先（Postgres の `live_lesson.sessions`）。

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
    live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
    scheduling::BookingId,
};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::pg_support::{is_unique_violation, timestamp};

/// [`LiveLessonSessionRepository`] の Postgres 実装。
///
/// 「同じ予約のセッションは 1 つ」は DB の `sessions_booking_id_key` で守る。終了は行のロック（`FOR UPDATE`）の
/// 中で読み取りから保存までを行う。
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

impl LiveLessonSessionRepository for PgLiveLessonSessionRepository {
    fn next_id(&self) -> SessionId {
        session_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn find(&self, session_id: SessionId) -> Result<Option<LiveLessonSession>, Unavailable> {
        let row: Option<SessionRow> = sqlx::query_as(select_sessions!("WHERE session_id = $1"))
            .bind(session_id_uuid::to_uuid(session_id))
            .fetch_optional(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        row.map(session_from_row).transpose()
    }

    async fn find_by_booking(
        &self,
        booking_id: BookingId,
    ) -> Result<Option<LiveLessonSession>, Unavailable> {
        let row: Option<SessionRow> = sqlx::query_as(select_sessions!("WHERE booking_id = $1"))
            .bind(booking_id_uuid::to_uuid(booking_id))
            .fetch_optional(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        row.map(session_from_row).transpose()
    }

    async fn insert(&self, session: LiveLessonSession) -> Result<(), InsertSessionError> {
        sqlx::query(
            "INSERT INTO live_lesson.sessions (session_id, booking_id, student_id, coach_id, started_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(session_id_uuid::to_uuid(session.id()))
        .bind(booking_id_uuid::to_uuid(session.booking_id()))
        .bind(account_id_uuid::to_uuid(session.student_id()))
        .bind(account_id_uuid::to_uuid(session.coach_id()))
        .bind(timestamp(session.started_at()))
        .execute(&self.pool)
        .await
        .map_err(|error| {
            if is_unique_violation(&error, "sessions_booking_id_key") {
                InsertSessionError::BookingAlreadyHasSession
            } else {
                InsertSessionError::Unavailable(Unavailable::new(error))
            }
        })?;
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
        let mut session = row
            .ok_or(UpdateSessionError::NotFound)
            .and_then(|row| session_from_row(row).map_err(UpdateSessionError::Unavailable))?;
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
        started_at.into(),
        ended,
    )
    .map_err(Unavailable::new)
}
