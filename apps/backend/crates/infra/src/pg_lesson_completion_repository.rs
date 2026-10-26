//! レッスンの完了記録の保存先（Postgres の `live_lesson.lesson_completions`・`async_lesson.lesson_completions`）。

use application::{
    Unavailable,
    async_lesson::video_id_uuid,
    identity::account_id_uuid,
    lesson::{LessonCompletionRepository, RecordCompletionError, lesson_id_uuid},
    live_lesson::session_id_uuid,
};
use domain::{
    DomainError,
    identity::AccountId,
    lesson::{LessonCompletion, LessonId, LessonSource},
};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::pg_support::{lock, timestamp};

/// [`LessonCompletionRepository`] の Postgres 実装。
///
/// 完了記録は元の集約のスキーマに置く（同期は `live_lesson`、非同期は `async_lesson`。docs/adr/0006）。2 つの
/// テーブルにまたがる「生徒の初回は 1 件」を守るため、読み取りから保存までを生徒ごとの advisory lock で直列にする
/// （ADR 0004 の 4）。
#[derive(Debug, Clone)]
pub struct PgLessonCompletionRepository {
    pool: PgPool,
}

/// 完了記録の 1 行（lesson_id, 元の ID（session_id / video_id）, coach_id, is_first_lesson, completed_at、
/// 同期なら true）。
type CompletionRow = (Uuid, Uuid, Uuid, bool, OffsetDateTime, bool);

impl PgLessonCompletionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl LessonCompletionRepository for PgLessonCompletionRepository {
    fn next_id(&self) -> LessonId {
        lesson_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn record<F>(
        &self,
        student_id: AccountId,
        record: F,
    ) -> Result<LessonCompletion, RecordCompletionError>
    where
        F: FnOnce(&[LessonCompletion]) -> Result<LessonCompletion, DomainError> + Send,
    {
        let unavailable = |e: sqlx::Error| RecordCompletionError::Unavailable(Unavailable::new(e));
        let student = account_id_uuid::to_uuid(student_id);
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        lock(&mut tx, &format!("lesson_completions:{student}"))
            .await
            .map_err(unavailable)?;
        let rows: Vec<CompletionRow> = sqlx::query_as(
            "SELECT lesson_id, session_id, coach_id, is_first_lesson, completed_at, true
             FROM live_lesson.lesson_completions WHERE student_id = $1
             UNION ALL
             SELECT lesson_id, video_id, coach_id, is_first_lesson, completed_at, false
             FROM async_lesson.lesson_completions WHERE student_id = $1",
        )
        .bind(student)
        .fetch_all(&mut *tx)
        .await
        .map_err(unavailable)?;
        let history: Vec<LessonCompletion> = rows
            .into_iter()
            .map(
                |(lesson_id, source_id, coach_id, is_first, completed_at, live)| {
                    let source = if live {
                        LessonSource::Live(session_id_uuid::from_uuid(source_id))
                    } else {
                        LessonSource::Async(video_id_uuid::from_uuid(source_id))
                    };
                    LessonCompletion::restore(
                        lesson_id_uuid::from_uuid(lesson_id),
                        student_id,
                        account_id_uuid::from_uuid(coach_id),
                        source,
                        completed_at.into(),
                        is_first,
                    )
                },
            )
            .collect();
        let completion = record(&history).map_err(RecordCompletionError::Rejected)?;

        let insert = match completion.source() {
            LessonSource::Live(session_id) => sqlx::query(
                "INSERT INTO live_lesson.lesson_completions
                     (lesson_id, session_id, student_id, coach_id, is_first_lesson, completed_at)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(lesson_id_uuid::to_uuid(completion.id()))
            .bind(session_id_uuid::to_uuid(session_id)),
            LessonSource::Async(video_id) => sqlx::query(
                "INSERT INTO async_lesson.lesson_completions
                     (lesson_id, video_id, student_id, coach_id, is_first_lesson, completed_at)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(lesson_id_uuid::to_uuid(completion.id()))
            .bind(video_id_uuid::to_uuid(video_id)),
        };
        insert
            .bind(account_id_uuid::to_uuid(completion.student_id()))
            .bind(account_id_uuid::to_uuid(completion.coach_id()))
            .bind(completion.is_first_lesson())
            .bind(timestamp(completion.completed_at()))
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(completion)
    }
}
