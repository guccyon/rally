//! `PgVideoRepository`（`async_lesson.videos`・`video_reviews`）の検証。送付・返却の往復と、2 回目の返却の拒否と、
//! コーチに送られた動画の列挙と、AI の一次フィードバックの往復（添削の返却で変わらないこと）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::async_lesson::{UpdateVideoError, VideoRepository};
use domain::{
    DomainError,
    async_lesson::{AiFeedback, ReviewComment, Video, VideoStatus},
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
        application::profile::ProfileStudentDisplayNameLookup::new(
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn ai_feedback_round_trips_and_survives_the_coachs_review(pool: PgPool) {
    let videos = PgVideoRepository::new(pool.clone());
    let (student, coach) = (support::student(&pool).await, support::coach(&pool).await);
    let video = Video::submit(
        videos.next_video_id(),
        student,
        coach,
        Sport::Swimming,
        at(1),
    );
    videos.insert(video.clone()).await.unwrap();
    assert_eq!(
        videos
            .find(video.id())
            .await
            .unwrap()
            .unwrap()
            .ai_feedback(),
        None,
        "付けるまでは空"
    );

    let feedback = AiFeedback::new("呼吸のタイミングを見直しましょう", at(1)).unwrap();
    let with_feedback = videos
        .update(video.id(), |v| v.record_ai_feedback(feedback.clone()))
        .await
        .unwrap();
    assert_eq!(with_feedback.ai_feedback(), Some(&feedback));
    assert_eq!(
        videos.find(video.id()).await.unwrap(),
        Some(with_feedback.clone())
    );

    // 2 回目は付けられず、保存した一次フィードバックは変わらない
    let result = videos
        .update(video.id(), |v| {
            v.record_ai_feedback(AiFeedback::new("別の文", at(2)).unwrap())
        })
        .await;
    assert!(
        matches!(
            result,
            Err(UpdateVideoError::Rejected(
                DomainError::AiFeedbackAlreadyRecorded
            ))
        ),
        "{result:?}"
    );

    // コーチの添削の返却は一次フィードバックを上書きしない（別の項目）
    let comment = ReviewComment::new("キックが膝から曲がっている").unwrap();
    let review_id = videos.next_review_id();
    let reviewed = videos
        .update(video.id(), |v| {
            v.return_review(review_id, coach, comment.clone(), true, at(5))
        })
        .await
        .unwrap();
    assert_eq!(reviewed.ai_feedback(), Some(&feedback));
    assert_eq!(reviewed.review().unwrap().comment(), &comment);
    let found = videos.find(video.id()).await.unwrap().unwrap();
    assert_eq!(found, reviewed);
    assert_eq!(
        videos.videos_of_coach(coach).await.unwrap(),
        [reviewed],
        "コーチの一覧でも読める"
    );
}
