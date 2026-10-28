//! `PgVideoRepository`（`async_lesson.videos`・`video_reviews`）の検証。送付・返却の往復と、2 回目の返却の拒否と、
//! コーチに送られた動画の列挙。
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn videos_of_coach_reads_videos_sent_to_the_coach_with_their_reviews(pool: PgPool) {
    let videos = PgVideoRepository::new(pool.clone());
    let (coach, other_coach) = (support::coach(&pool).await, support::coach(&pool).await);
    let student = support::student(&pool).await;
    let submit = |coach, hour| {
        Video::submit(
            videos.next_video_id(),
            student,
            coach,
            Sport::Tennis,
            at(hour),
        )
    };
    let (older, newer, returned, others) = (
        submit(coach, 2),
        submit(coach, 3),
        submit(coach, 1),
        submit(other_coach, 1),
    );
    for video in [&older, &newer, &returned, &others] {
        videos.insert(video.clone()).await.unwrap();
    }
    let returned = videos
        .update(returned.id(), |v| {
            v.return_review(
                videos.next_review_id(),
                coach,
                ReviewComment::new("テイクバックが早い").unwrap(),
                true,
                at(4),
            )
        })
        .await
        .unwrap();

    let mut found = videos.videos_of_coach(coach).await.unwrap();
    found.sort_by_key(Video::submitted_at);
    assert_eq!(
        found,
        [returned.clone(), older.clone(), newer.clone()],
        "返却した添削も読み、別のコーチに送られた動画は読まない"
    );

    // ユースケースを通すと、添削待ちが送られた時刻の古い順で先、返却済みが後で、生徒の表示名が添わる
    let list = application::async_lesson::ListCoachVideos::new(
        videos.clone(),
        application::identity::IdentityStudentDisplayNameLookup::new(
            infra::PgAccountRepository::new(pool.clone()),
        ),
    )
    .execute(coach)
    .await
    .unwrap();
    assert_eq!(list.videos, [older, newer, returned]);
    assert_eq!(
        list.students.get(&student).map(|name| name.as_str()),
        Some("テスト")
    );

    let nobody = support::coach(&pool).await;
    assert!(videos.videos_of_coach(nobody).await.unwrap().is_empty());
}
