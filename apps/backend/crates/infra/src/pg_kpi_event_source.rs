//! KPI の集計が読む行動イベントを、Postgres の業務の保存先から組み立てる（Gen1 は行動ログを貯めていないため）。

use application::{
    Unavailable,
    analytics::{KpiAttribute, KpiEvent, KpiEventSource},
};
use sqlx::{PgPool, Row, postgres::PgRow};
use time::OffsetDateTime;
use uuid::Uuid;

/// [`KpiEventSource`] の Postgres の実装。Gen1 は行動イベント（events.yaml）を staging に貯めていない
/// （`ANALYTICS_SINK` は既定のプロセス内）ので、同じ事実を持つ業務の表から、イベント名ごとに events.yaml の
/// `required` の属性の行を組み立てる。KPI の式（kpi.yaml）はここに持たず、イベントを写すだけ。
///
/// 業務の表に残っていない属性は null（`slot.withdrawn` の `occurred_at`。取り下げた時刻は残さない）。
/// `slot.withdrawn` は、閉じた枠のうち有効な予約がなく、コーチのお休みで本部が予約をキャンセルした枠でもないもの
/// （取り下げは予約の入っていない枠だけ。`WithdrawSlot`）。知らないイベント名は [`Unavailable`]。
#[derive(Debug, Clone)]
pub struct PgKpiEventSource {
    pool: PgPool,
}

#[derive(Clone, Copy)]
enum Kind {
    Id,
    Bool,
    Number,
    Time,
    Text,
}

/// イベント名ごとの SQL と、列（属性の名前）の型。数の列は SQL で `float8` にそろえる。
fn query(name: &str) -> Option<(&'static str, &'static [(&'static str, Kind)])> {
    use Kind::{Bool, Id, Number, Text, Time};
    Some(match name {
        "slot.published" => (
            "SELECT coach_id, slot_id, starts_at, created_at AS occurred_at FROM scheduling.slots",
            &[("coach_id", Id), ("slot_id", Id), ("starts_at", Time), ("occurred_at", Time)],
        ),
        "slot.withdrawn" => (
            "SELECT s.coach_id, s.slot_id, s.starts_at, NULL::timestamptz AS occurred_at
             FROM scheduling.slots s
             WHERE s.status = 'closed'
               AND NOT EXISTS (SELECT 1 FROM scheduling.bookings b
                               WHERE b.slot_id = s.slot_id AND b.status <> 'cancelled')
               AND NOT EXISTS (SELECT 1 FROM scheduling.bookings b
                               JOIN scheduling.coach_leave_bookings l ON l.booking_id = b.booking_id
                               WHERE b.slot_id = s.slot_id)",
            &[("coach_id", Id), ("slot_id", Id), ("starts_at", Time), ("occurred_at", Time)],
        ),
        "booking.created" => (
            "SELECT b.student_id, s.coach_id, b.booking_id, b.slot_id, b.is_trial, b.created_at AS occurred_at
             FROM scheduling.bookings b JOIN scheduling.slots s ON s.slot_id = b.slot_id",
            &[
                ("student_id", Id),
                ("coach_id", Id),
                ("booking_id", Id),
                ("slot_id", Id),
                ("is_trial", Bool),
                ("occurred_at", Time),
            ],
        ),
        "booking.confirmed" => (
            "SELECT b.student_id, s.coach_id, b.booking_id, b.slot_id, b.confirmed_at AS occurred_at
             FROM scheduling.bookings b JOIN scheduling.slots s ON s.slot_id = b.slot_id
             WHERE b.confirmed_at IS NOT NULL",
            &[
                ("student_id", Id),
                ("coach_id", Id),
                ("booking_id", Id),
                ("slot_id", Id),
                ("occurred_at", Time),
            ],
        ),
        "booking.cancelled" => (
            "SELECT b.student_id, s.coach_id, b.booking_id, b.slot_id,
                    trunc(extract(epoch FROM s.starts_at - b.cancelled_at) / 3600)::float8 AS hours_before_start,
                    b.cancelled_at AS occurred_at
             FROM scheduling.bookings b JOIN scheduling.slots s ON s.slot_id = b.slot_id
             WHERE b.status = 'cancelled'",
            &[
                ("student_id", Id),
                ("coach_id", Id),
                ("booking_id", Id),
                ("slot_id", Id),
                ("hours_before_start", Number),
                ("occurred_at", Time),
            ],
        ),
        "call.ended" => (
            "SELECT student_id, coach_id, booking_id, session_id,
                    trunc(extract(epoch FROM ended_at - started_at))::float8 AS duration_sec,
                    packet_loss_rate, jitter_ms::float8 AS jitter_ms, round_trip_ms::float8 AS round_trip_ms,
                    disconnect_count::float8 AS disconnect_count, ended_at AS occurred_at
             FROM live_lesson.sessions WHERE ended_at IS NOT NULL",
            &[
                ("student_id", Id),
                ("coach_id", Id),
                ("booking_id", Id),
                ("session_id", Id),
                ("duration_sec", Number),
                ("packet_loss_rate", Number),
                ("jitter_ms", Number),
                ("round_trip_ms", Number),
                ("disconnect_count", Number),
                ("occurred_at", Time),
            ],
        ),
        "video.submitted" => (
            "SELECT student_id, coach_id, video_id, sport, submitted_at AS occurred_at FROM async_lesson.videos",
            &[
                ("student_id", Id),
                ("coach_id", Id),
                ("video_id", Id),
                ("sport", Text),
                ("occurred_at", Time),
            ],
        ),
        "video_review.returned" => (
            "SELECT v.student_id, v.coach_id, r.video_id, r.review_id, v.submitted_at, r.returned_at AS occurred_at
             FROM async_lesson.video_reviews r JOIN async_lesson.videos v ON v.video_id = r.video_id",
            &[
                ("student_id", Id),
                ("coach_id", Id),
                ("video_id", Id),
                ("review_id", Id),
                ("submitted_at", Time),
                ("occurred_at", Time),
            ],
        ),
        "lesson.completed" => (
            "SELECT student_id, coach_id, lesson_id, 'live_lesson' AS lesson_type, is_first_lesson,
                    completed_at AS occurred_at
             FROM live_lesson.lesson_completions
             UNION ALL
             SELECT student_id, coach_id, lesson_id, 'async_lesson' AS lesson_type, is_first_lesson,
                    completed_at AS occurred_at
             FROM async_lesson.lesson_completions",
            &[
                ("student_id", Id),
                ("coach_id", Id),
                ("lesson_id", Id),
                ("lesson_type", Text),
                ("is_first_lesson", Bool),
                ("occurred_at", Time),
            ],
        ),
        "subscription.started" => (
            "SELECT student_id, subscription_id, sport, started_at AS occurred_at FROM entitlement.subscriptions",
            &[
                ("student_id", Id),
                ("subscription_id", Id),
                ("sport", Text),
                ("occurred_at", Time),
            ],
        ),
        _ => return None,
    })
}

fn attribute(row: &PgRow, column: &str, kind: Kind) -> Result<KpiAttribute, sqlx::Error> {
    Ok(match kind {
        Kind::Id => row
            .try_get::<Option<Uuid>, _>(column)?
            .map(|id| id.hyphenated().to_string())
            .into(),
        Kind::Bool => row.try_get::<Option<bool>, _>(column)?.into(),
        Kind::Number => row.try_get::<Option<f64>, _>(column)?.into(),
        Kind::Time => row
            .try_get::<Option<OffsetDateTime>, _>(column)?
            .map(std::time::SystemTime::from)
            .into(),
        Kind::Text => row.try_get::<Option<String>, _>(column)?.into(),
    })
}

impl PgKpiEventSource {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl KpiEventSource for PgKpiEventSource {
    async fn events(&self, name: &str) -> Result<Vec<KpiEvent>, Unavailable> {
        let (sql, columns) = query(name)
            .ok_or_else(|| Unavailable::new(format!("{name} を業務の表から組み立てられない")))?;
        let rows = sqlx::query(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(Unavailable::new)?;
        rows.iter()
            .map(|row| {
                columns
                    .iter()
                    .try_fold(KpiEvent::new(name), |event, (column, kind)| {
                        Ok(event.with(*column, attribute(row, column, *kind)?))
                    })
            })
            .collect::<Result<_, sqlx::Error>>()
            .map_err(Unavailable::new)
    }
}
