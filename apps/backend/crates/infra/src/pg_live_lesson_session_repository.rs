//! 通話セッションの保存先（Postgres の `live_lesson.sessions` と、結び付いた予約の `live_lesson.session_bookings`、
//! 本部が終わらせた記録の `live_lesson.session_headquarters_ends`）。

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
    identity::AccountId,
    live_lesson::{ConnectionQuality, LiveLessonSession, SessionBooking, SessionId},
    scheduling::BookingId,
};
use std::time::SystemTime;

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
///
/// 本部が終わらせたセッション（[`LiveLessonSession::ended_by_headquarters`]）は、`sessions` の行を変えずに
/// `session_headquarters_ends` に 1 行足す（`sessions` は終了の時刻と接続品質を揃えて持つ制約があり、本部の終了には
/// 指標がない）。読むときは `sessions.ended_at` があれば参加者の終了、なければ `session_headquarters_ends` の行が
/// 本部の終了。両方あるのは古い版に戻した間に参加者が終了したときだけで、そのときは参加者の終了を採る。
#[derive(Debug, Clone)]
pub struct PgLiveLessonSessionRepository {
    pool: PgPool,
}

/// `live_lesson.sessions` の 1 行（session_id, booking_id, student_id, coach_id, started_at, ended_at,
/// packet_loss_rate, jitter_ms, round_trip_ms, disconnect_count）と、本部が終わらせた記録
/// （`session_headquarters_ends` の ended_by, ended_at。なければ NULL）。
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
    Option<Uuid>,
    Option<OffsetDateTime>,
);

/// セッションを読む SELECT（`$where` は WHERE 句から後ろ。`sessions` は `s`、本部が終わらせた記録は `h`）。
macro_rules! select_sessions {
    ($where:literal) => {
        concat!(
            "SELECT s.session_id, s.booking_id, s.student_id, s.coach_id, s.started_at, s.ended_at,
                    s.packet_loss_rate, s.jitter_ms, s.round_trip_ms, s.disconnect_count,
                    h.ended_by, h.ended_at
             FROM live_lesson.sessions s
             LEFT JOIN live_lesson.session_headquarters_ends h ON h.session_id = s.session_id ",
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
        let row: Option<SessionRow> = sqlx::query_as(select_sessions!("WHERE s.session_id = $1"))
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
            "WHERE s.booking_id = $1
                OR s.session_id IN (SELECT session_id FROM live_lesson.session_bookings WHERE booking_id = $1)"
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

    async fn find_in_progress_started_at_or_before(
        &self,
        started_at_or_before: SystemTime,
    ) -> Result<Vec<LiveLessonSession>, Unavailable> {
        let mut conn = self.pool.acquire().await.map_err(Unavailable::new)?;
        let rows: Vec<SessionRow> = sqlx::query_as(select_sessions!(
            "WHERE s.ended_at IS NULL AND h.session_id IS NULL AND s.started_at <= $1
             ORDER BY s.started_at, s.session_id"
        ))
        .bind(timestamp(started_at_or_before))
        .fetch_all(&mut *conn)
        .await
        .map_err(Unavailable::new)?;
        let mut sessions = Vec::with_capacity(rows.len());
        for row in rows {
            sessions.push(session_with_bookings(&mut conn, row).await?);
        }
        Ok(sessions)
    }

    async fn find_started_between(
        &self,
        starts_at: SystemTime,
        ends_at: SystemTime,
        coach_id: Option<AccountId>,
    ) -> Result<Vec<LiveLessonSession>, Unavailable> {
        let mut conn = self.pool.acquire().await.map_err(Unavailable::new)?;
        let rows: Vec<SessionRow> = sqlx::query_as(select_sessions!(
            "WHERE s.started_at >= $1 AND s.started_at < $2 AND ($3::uuid IS NULL OR s.coach_id = $3)
             ORDER BY s.started_at DESC, s.session_id"
        ))
        .bind(timestamp(starts_at))
        .bind(timestamp(ends_at))
        .bind(coach_id.map(account_id_uuid::to_uuid))
        .fetch_all(&mut *conn)
        .await
        .map_err(Unavailable::new)?;
        let mut sessions = Vec::with_capacity(rows.len());
        for row in rows {
            sessions.push(session_with_bookings(&mut conn, row).await?);
        }
        Ok(sessions)
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
            sqlx::query_as(select_sessions!("WHERE s.session_id = $1 FOR UPDATE OF s"))
                .bind(session_id_uuid::to_uuid(session_id))
                .fetch_optional(&mut *tx)
                .await
                .map_err(unavailable)?;
        let row = row.ok_or(UpdateSessionError::NotFound)?;
        let mut session = session_with_bookings(&mut tx, row)
            .await
            .map_err(UpdateSessionError::Unavailable)?;
        update(&mut session).map_err(UpdateSessionError::Rejected)?;

        if let Some(ended_by) = session.ended_by_headquarters() {
            // 本部の終了は sessions の行を変えずに記録を足す（2 回目は行のロックの中で domain が拒む）
            let ended_at = session
                .ended_at()
                .ok_or_else(|| Unavailable::new("本部が終わらせたセッションに終了時刻がない"))
                .map_err(UpdateSessionError::Unavailable)?;
            sqlx::query(
                "INSERT INTO live_lesson.session_headquarters_ends (session_id, ended_by, ended_at)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (session_id) DO NOTHING",
            )
            .bind(session_id_uuid::to_uuid(session.id()))
            .bind(account_id_uuid::to_uuid(ended_by))
            .bind(timestamp(ended_at))
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
            tx.commit().await.map_err(unavailable)?;
            return Ok(session);
        }

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
        headquarters_ended_by,
        headquarters_ended_at,
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
    let mut session = LiveLessonSession::restore(
        session_id_uuid::from_uuid(session_id),
        booking_id_uuid::from_uuid(booking_id),
        account_id_uuid::from_uuid(student_id),
        account_id_uuid::from_uuid(coach_id),
        bookings,
        started_at.into(),
        ended,
    )
    .map_err(Unavailable::new)?;
    // 参加者の終了があればそちらを採る（両方あるのは古い版に戻した間に参加者が終了したときだけ）
    if let (None, Some(ended_by), Some(ended_at)) =
        (ended, headquarters_ended_by, headquarters_ended_at)
    {
        session
            .end_by_headquarters(account_id_uuid::from_uuid(ended_by), ended_at.into())
            .map_err(Unavailable::new)?;
    }
    Ok(session)
}
