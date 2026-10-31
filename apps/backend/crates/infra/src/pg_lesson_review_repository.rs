//! レッスンの評価の保存先（Postgres の `review.lesson_reviews`）。

use application::{
    Unavailable,
    identity::account_id_uuid,
    lesson::lesson_completion_id_uuid,
    review::{InsertReviewError, LessonReviewRepository, lesson_review_id_uuid},
};
use domain::{
    identity::AccountId,
    lesson::LessonCompletionId,
    review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::pg_support::{is_unique_violation, timestamp};

/// [`LessonReviewRepository`] の Postgres 実装。
///
/// 「完了したレッスン 1 件につき評価は 1 つ」は DB の `lesson_reviews_lesson_id_key` で守り、並行な評価の後の方は
/// [`InsertReviewError::LessonAlreadyReviewed`] になる。
#[derive(Debug, Clone)]
pub struct PgLessonReviewRepository {
    pool: PgPool,
}

/// 評価の列（review_id, lesson_id, student_id, coach_id, rating, comment, submitted_at）。
type ReviewRow = (Uuid, Uuid, Uuid, Uuid, i16, String, OffsetDateTime);

impl PgLessonReviewRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl LessonReviewRepository for PgLessonReviewRepository {
    fn next_id(&self) -> LessonReviewId {
        lesson_review_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn find_by_lesson(
        &self,
        lesson_id: LessonCompletionId,
    ) -> Result<Option<LessonReview>, Unavailable> {
        let row: Option<ReviewRow> = sqlx::query_as(
            "SELECT review_id, lesson_id, student_id, coach_id, rating, comment, submitted_at
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

    async fn find_by_coach(&self, coach_id: AccountId) -> Result<Vec<LessonReview>, Unavailable> {
        let rows: Vec<ReviewRow> = sqlx::query_as(
            "SELECT review_id, lesson_id, student_id, coach_id, rating, comment, submitted_at
             FROM review.lesson_reviews WHERE coach_id = $1 ORDER BY submitted_at",
        )
        .bind(account_id_uuid::to_uuid(coach_id))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        rows.into_iter().map(review_from_row).collect()
    }
}

fn review_from_row(
    (review_id, lesson_id, student_id, coach_id, rating, comment, submitted_at): ReviewRow,
) -> Result<LessonReview, Unavailable> {
    Ok(LessonReview::restore(
        lesson_review_id_uuid::from_uuid(review_id),
        lesson_completion_id_uuid::from_uuid(lesson_id),
        account_id_uuid::from_uuid(student_id),
        account_id_uuid::from_uuid(coach_id),
        Rating::new(i64::from(rating)).map_err(Unavailable::new)?,
        LessonReviewComment::new(comment).map_err(Unavailable::new)?,
        submitted_at.into(),
    ))
}
