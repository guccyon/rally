//! `PgVideoRepository`（`async_lesson.videos`・`video_reviews`）の検証。送付・返却の往復と、2 回目の返却の拒否。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::async_lesson::{UpdateVideoError, VideoRepository};
use domain::{
    DomainError,
    async_lesson::{ReviewComment, Video, VideoStatus},
    identity::Sport,
};
use infra::PgVideoRepository;
use sqlx::PgPool;
use support::at;

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn video_round_trips_through_submit_and_review(pool: PgPool) {
    let videos = PgVideoRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let video = Video::submit(videos.next_video_id(), student, coach, Sport::Tennis, at(1));
    videos.insert(video.clone()).await.unwrap();
    assert_eq!(videos.find(video.id()).await.unwrap(), Some(video.clone()));

    let comment = ReviewComment::new("テイクバックが早い").unwrap();
    let review_id = videos.next_review_id();
    let reviewed = videos
        .update(video.id(), |v| {
            v.return_review(review_id, coach, comment.clone(), true, at(5))
        })
        .await
        .unwrap();
    assert_eq!(reviewed.status(), VideoStatus::Reviewed);
    assert_eq!(videos.find(video.id()).await.unwrap(), Some(reviewed));

    let result = videos
        .update(video.id(), |v| {
            v.return_review(videos.next_review_id(), coach, comment.clone(), true, at(6))
        })
        .await;
    assert!(
        matches!(
            result,
            Err(UpdateVideoError::Rejected(
                DomainError::VideoReviewAlreadyReturned
            ))
        ),
        "{result:?}"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM async_lesson.video_reviews")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!(matches!(
        videos.update(videos.next_video_id(), |_| Ok(())).await,
        Err(UpdateVideoError::NotFound)
    ));
}
