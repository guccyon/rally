//! 動画と添削の保存先（Postgres の `async_lesson.videos`・`video_reviews`）。AI の一次フィードバックと、送った時点の
//! レッスンメニューの写しは `videos` の列。

use application::{
    Unavailable,
    async_lesson::{UpdateVideoError, VideoRepository, video_id_uuid, video_review_id_uuid},
    catalog::lesson_menu_id_uuid,
    identity::account_id_uuid,
};
use domain::{
    DomainError,
    async_lesson::{AiFeedback, ReviewComment, Video, VideoId, VideoLessonMenu, VideoReviewId},
    catalog::LessonMenuName,
    identity::AccountId,
};
use sqlx::{
    PgPool,
    types::{Uuid, time::OffsetDateTime},
};

use crate::{
    identity_values::{sport_from_value, sport_value},
    pg_support::timestamp,
};

/// [`VideoRepository`] の Postgres 実装。
///
/// 動画の状態は添削の行の有無で決まる。返却は動画の行のロック（`FOR UPDATE`）の中で読み取りから保存までを行い、
/// 「返却は 1 回」は DB の `video_reviews_video_id_key` でも守る。AI の一次フィードバックは動画の行の
/// `ai_feedback_comment`・`ai_feedback_created_at` に書き、添削の行とは別に持つ（添削の返却では書き換えない）。
#[derive(Debug, Clone)]
pub struct PgVideoRepository {
    pool: PgPool,
}

/// 動画の列（video_id, student_id, coach_id, sport, submitted_at）と、添削の列（review_id, comment, returned_at）と、
/// AI の一次フィードバックの列（ai_feedback_comment, ai_feedback_created_at）と、送った時点のレッスンメニューの写しの列
/// （lesson_menu_id, lesson_menu_name。写しのない動画はどちらも NULL。DB の `videos_lesson_menu_shape`）。
type VideoRow = (
    Uuid,
    Uuid,
    Uuid,
    String,
    OffsetDateTime,
    Option<Uuid>,
    Option<String>,
    Option<OffsetDateTime>,
    Option<String>,
    Option<OffsetDateTime>,
    Option<Uuid>,
    Option<String>,
);

impl PgVideoRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl VideoRepository for PgVideoRepository {
    fn next_video_id(&self) -> VideoId {
        video_id_uuid::from_uuid(Uuid::new_v4())
    }

    fn next_review_id(&self) -> VideoReviewId {
        video_review_id_uuid::from_uuid(Uuid::new_v4())
    }

    async fn find(&self, video_id: VideoId) -> Result<Option<Video>, Unavailable> {
        let row: Option<VideoRow> = sqlx::query_as(
            "SELECT v.video_id, v.student_id, v.coach_id, v.sport, v.submitted_at,
                    r.review_id, r.comment, r.returned_at,
                    v.ai_feedback_comment, v.ai_feedback_created_at,
                    v.lesson_menu_id, v.lesson_menu_name
             FROM async_lesson.videos v
             LEFT JOIN async_lesson.video_reviews r ON r.video_id = v.video_id
             WHERE v.video_id = $1",
        )
        .bind(video_id_uuid::to_uuid(video_id))
        .fetch_optional(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        row.map(video_from_row).transpose()
    }

    async fn videos_of_coach(&self, coach_id: AccountId) -> Result<Vec<Video>, Unavailable> {
        let rows: Vec<VideoRow> = sqlx::query_as(
            "SELECT v.video_id, v.student_id, v.coach_id, v.sport, v.submitted_at,
                    r.review_id, r.comment, r.returned_at,
                    v.ai_feedback_comment, v.ai_feedback_created_at,
                    v.lesson_menu_id, v.lesson_menu_name
             FROM async_lesson.videos v
             LEFT JOIN async_lesson.video_reviews r ON r.video_id = v.video_id
             WHERE v.coach_id = $1",
        )
        .bind(account_id_uuid::to_uuid(coach_id))
        .fetch_all(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        rows.into_iter().map(video_from_row).collect()
    }

    async fn insert(&self, video: Video) -> Result<(), Unavailable> {
        sqlx::query(
            "INSERT INTO async_lesson.videos
                 (video_id, student_id, coach_id, sport, submitted_at, lesson_menu_id, lesson_menu_name)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(video_id_uuid::to_uuid(video.id()))
        .bind(account_id_uuid::to_uuid(video.student_id()))
        .bind(account_id_uuid::to_uuid(video.coach_id()))
        .bind(sport_value(video.sport()))
        .bind(timestamp(video.submitted_at()))
        .bind(
            video
                .lesson_menu()
                .map(|menu| lesson_menu_id_uuid::to_uuid(menu.id())),
        )
        .bind(video.lesson_menu().map(|menu| menu.name().as_str()))
        .execute(&self.pool)
        .await
        .map_err(Unavailable::new)?;
        Ok(())
    }

    async fn update<F>(&self, video_id: VideoId, update: F) -> Result<Video, UpdateVideoError>
    where
        F: FnOnce(&mut Video) -> Result<(), DomainError> + Send,
    {
        let unavailable = |e: sqlx::Error| UpdateVideoError::Unavailable(Unavailable::new(e));
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let row: Option<VideoRow> = sqlx::query_as(
            "SELECT v.video_id, v.student_id, v.coach_id, v.sport, v.submitted_at,
                    r.review_id, r.comment, r.returned_at,
                    v.ai_feedback_comment, v.ai_feedback_created_at,
                    v.lesson_menu_id, v.lesson_menu_name
             FROM async_lesson.videos v
             LEFT JOIN async_lesson.video_reviews r ON r.video_id = v.video_id
             WHERE v.video_id = $1
             FOR UPDATE OF v",
        )
        .bind(video_id_uuid::to_uuid(video_id))
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?;
        let mut video = row
            .ok_or(UpdateVideoError::NotFound)
            .and_then(|row| video_from_row(row).map_err(UpdateVideoError::Unavailable))?;
        let had_review = video.review().is_some();
        let had_ai_feedback = video.ai_feedback().is_some();
        update(&mut video).map_err(UpdateVideoError::Rejected)?;

        // 動画が変わるのは添削の返却と AI の一次フィードバックを付けることだけ（どちらも付けた後は変わらない。
        // レッスンメニューの写しは送付のときに書いたまま変わらない）
        if let (false, Some(review)) = (had_review, video.review()) {
            sqlx::query(
                "INSERT INTO async_lesson.video_reviews (review_id, video_id, comment, returned_at)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(video_review_id_uuid::to_uuid(review.id()))
            .bind(video_id_uuid::to_uuid(video.id()))
            .bind(review.comment().as_str())
            .bind(timestamp(review.returned_at()))
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        }
        if let (false, Some(feedback)) = (had_ai_feedback, video.ai_feedback()) {
            sqlx::query(
                "UPDATE async_lesson.videos
                 SET ai_feedback_comment = $2, ai_feedback_created_at = $3
                 WHERE video_id = $1",
            )
            .bind(video_id_uuid::to_uuid(video.id()))
            .bind(feedback.comment())
            .bind(timestamp(feedback.created_at()))
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(video)
    }
}

fn video_from_row(
    (
        video_id,
        student_id,
        coach_id,
        sport,
        submitted_at,
        review_id,
        comment,
        returned_at,
        ai_feedback_comment,
        ai_feedback_created_at,
        lesson_menu_id,
        lesson_menu_name,
    ): VideoRow,
) -> Result<Video, Unavailable> {
    let sport = sport_from_value(&sport)
        .ok_or_else(|| Unavailable::new(format!("未知の sport: {sport}")))?;
    let review = match (review_id, comment, returned_at) {
        (Some(review_id), Some(comment), Some(returned_at)) => Some((
            video_review_id_uuid::from_uuid(review_id),
            ReviewComment::new(comment).map_err(Unavailable::new)?,
            returned_at.into(),
        )),
        _ => None,
    };
    let mut video = Video::restore(
        video_id_uuid::from_uuid(video_id),
        account_id_uuid::from_uuid(student_id),
        account_id_uuid::from_uuid(coach_id),
        sport,
        submitted_at.into(),
        review,
    )
    .map_err(Unavailable::new)?
    .with_restored_lesson_menu(lesson_menu_from_columns(lesson_menu_id, lesson_menu_name)?);
    if let (Some(comment), Some(created_at)) = (ai_feedback_comment, ai_feedback_created_at) {
        video
            .record_ai_feedback(
                AiFeedback::new(comment, created_at.into()).map_err(Unavailable::new)?,
            )
            .map_err(Unavailable::new)?;
    }
    Ok(video)
}

/// 送った時点のレッスンメニューの写し。写しのない動画は `None`。
fn lesson_menu_from_columns(
    lesson_menu_id: Option<Uuid>,
    name: Option<String>,
) -> Result<Option<VideoLessonMenu>, Unavailable> {
    let (Some(lesson_menu_id), Some(name)) = (lesson_menu_id, name) else {
        return Ok(None);
    };
    Ok(Some(VideoLessonMenu::restore(
        lesson_menu_id_uuid::from_uuid(lesson_menu_id),
        LessonMenuName::new(name).map_err(Unavailable::new)?,
    )))
}
