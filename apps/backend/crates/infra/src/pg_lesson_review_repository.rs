//! レッスンの評価の保存先（Postgres の `review.lesson_reviews`）。

use std::time::SystemTime;

use application::{
    Unavailable,
    identity::account_id_uuid,
    lesson::lesson_completion_id_uuid,
    review::{InsertReviewError, LessonReviewRepository, lesson_review_id_uuid},
};
use domain::{
    identity::AccountId,
    lesson::LessonCompletionId,
    review::{
        LessonReview, LessonReviewComment, LessonReviewId, LessonReviewReply,
        LessonReviewReplyComment, LowRatedReviewHandling, Rating,
    },
};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::pg_support::{is_unique_violation, timestamp};

/// [`LessonReviewRepository`] の Postgres 実装。
///
/// 「完了したレッスン 1 件につき評価は 1 つ」は DB の `lesson_reviews_lesson_id_key` で守り、並行な評価の後の方は
/// [`InsertReviewError::LessonAlreadyReviewed`] になる。評価へのコーチの返信は同じ行の `reply_comment` / `replied_at`、
/// 本部の対応済みの印は `handled_by` / `handled_at`（最初の印だけを残す）。
#[derive(Debug, Clone)]
pub struct PgLessonReviewRepository {
    pool: PgPool,
}

/// 評価の列（review_id, lesson_id, student_id, coach_id, rating, comment, submitted_at, reply_comment, replied_at,
/// handled_by, handled_at）。
type ReviewRow = (
    Uuid,
    Uuid,
    Uuid,
    Uuid,
    i16,
    String,
    OffsetDateTime,
    Option<String>,
    Option<OffsetDateTime>,
    Option<Uuid>,
    Option<OffsetDateTime>,
);

impl PgLessonReviewRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl LessonReviewRepository for PgLessonReviewRepository {
    fn next_id(&self) -> LessonReviewId {
        lesson_review_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn find_by_id(
        &self,
        review_id: LessonReviewId,
    ) -> Result<Option<LessonReview>, Unavailable> {
        let row: Option<ReviewRow> = sqlx::query_as(
            "SELECT review_id, lesson_id, student_id, coach_id, rating, comment, submitted_at,
                    reply_comment, replied_at, handled_by, handled_at
             FROM review.lesson_reviews WHERE review_id = $1",
        )
        .bind(lesson_review_id_uuid::to_uuid(review_id))
        .fetch_optional(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        row.map(review_from_row).transpose()
    }

    async fn find_by_lesson(
        &self,
        lesson_id: LessonCompletionId,
    ) -> Result<Option<LessonReview>, Unavailable> {
        let row: Option<ReviewRow> = sqlx::query_as(
            "SELECT review_id, lesson_id, student_id, coach_id, rating, comment, submitted_at,
                    reply_comment, replied_at, handled_by, handled_at
             FROM review.lesson_reviews WHERE lesson_id = $1",
        )
        .bind(lesson_completion_id_uuid::to_uuid(lesson_id))
        .fetch_optional(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        row.map(review_from_row).transpose()
    }

    async fn insert(&self, review: LessonReview) -> Result<(), InsertReviewError> {
        sqlx::query(
            "INSERT INTO review.lesson_reviews
                 (review_id, lesson_id, student_id, coach_id, rating, comment, submitted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(lesson_review_id_uuid::to_uuid(review.id()))
        .bind(lesson_completion_id_uuid::to_uuid(review.lesson_id()))
        .bind(account_id_uuid::to_uuid(review.student_id()))
        .bind(account_id_uuid::to_uuid(review.coach_id()))
        .bind(i16::from(review.rating().value()))
        .bind(review.comment().as_str())
        .bind(timestamp(review.submitted_at()))
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e, "lesson_reviews_lesson_id_key") {
                InsertReviewError::LessonAlreadyReviewed
            } else {
                InsertReviewError::Unavailable(Unavailable::new(e))
            }
        })?;
        Ok(())
    }

    async fn save_reply(
        &self,
        review_id: LessonReviewId,
        reply: &LessonReviewReply,
    ) -> Result<(), Unavailable> {
        sqlx::query(
            "UPDATE review.lesson_reviews SET reply_comment = $2, replied_at = $3
             WHERE review_id = $1",
        )
        .bind(lesson_review_id_uuid::to_uuid(review_id))
        .bind(reply.comment().as_str())
        .bind(timestamp(reply.replied_at()))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn save_handling(
        &self,
        review_id: LessonReviewId,
        handling: &LowRatedReviewHandling,
    ) -> Result<(), Unavailable> {
        // 印がすでにあれば書かない（並行に付けても最初の印が残る）
        sqlx::query(
            "UPDATE review.lesson_reviews SET handled_by = $2, handled_at = $3
             WHERE review_id = $1 AND handled_at IS NULL",
        )
        .bind(lesson_review_id_uuid::to_uuid(review_id))
        .bind(account_id_uuid::to_uuid(handling.handled_by()))
        .bind(timestamp(handling.handled_at()))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn find_by_coach(&self, coach_id: AccountId) -> Result<Vec<LessonReview>, Unavailable> {
        let rows: Vec<ReviewRow> = sqlx::query_as(
            "SELECT review_id, lesson_id, student_id, coach_id, rating, comment, submitted_at,
                    reply_comment, replied_at, handled_by, handled_at
             FROM review.lesson_reviews WHERE coach_id = $1 ORDER BY submitted_at",
        )
        .bind(account_id_uuid::to_uuid(coach_id))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        rows.into_iter().map(review_from_row).collect()
    }

    async fn find_submitted_since(
        &self,
        since: SystemTime,
    ) -> Result<Vec<LessonReview>, Unavailable> {
        let rows: Vec<ReviewRow> = sqlx::query_as(
            "SELECT review_id, lesson_id, student_id, coach_id, rating, comment, submitted_at,
                    reply_comment, replied_at, handled_by, handled_at
             FROM review.lesson_reviews WHERE submitted_at >= $1 ORDER BY submitted_at",
        )
        .bind(timestamp(since))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        rows.into_iter().map(review_from_row).collect()
    }
}

fn review_from_row(
    (
        review_id,
        lesson_id,
        student_id,
        coach_id,
        rating,
        comment,
        submitted_at,
        reply_comment,
        replied_at,
        handled_by,
        handled_at,
    ): ReviewRow,
) -> Result<LessonReview, Unavailable> {
    // 文と時刻は揃って NULL か揃って値を持つ（DB の lesson_reviews_reply_shape）
    let reply = match (reply_comment, replied_at) {
        (Some(comment), Some(replied_at)) => Some(LessonReviewReply::new(
            LessonReviewReplyComment::new(comment).map_err(Unavailable::new)?,
            replied_at.into(),
        )),
        _ => None,
    };
    // アカウントと時刻は揃って NULL か揃って値を持つ（DB の lesson_reviews_handling_shape）
    let handling = match (handled_by, handled_at) {
        (Some(handled_by), Some(handled_at)) => Some(LowRatedReviewHandling::new(
            account_id_uuid::from_uuid(handled_by),
            handled_at.into(),
        )),
        _ => None,
    };
    Ok(LessonReview::restore(
        lesson_review_id_uuid::from_uuid(review_id),
        lesson_completion_id_uuid::from_uuid(lesson_id),
        account_id_uuid::from_uuid(student_id),
        account_id_uuid::from_uuid(coach_id),
        Rating::new(i64::from(rating)).map_err(Unavailable::new)?,
        LessonReviewComment::new(comment).map_err(Unavailable::new)?,
        submitted_at.into(),
    )
    .with_reply(reply)
    .with_handling(handling))
}
