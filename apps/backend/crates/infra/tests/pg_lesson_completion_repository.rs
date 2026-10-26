//! `PgLessonCompletionRepository`（`live_lesson.lesson_completions`・`async_lesson.lesson_completions`）の検証。
//! 同期・非同期をまたいだ初回の判定、同じレッスンの 2 回目の拒否、並行な記録で初回が 1 件になること。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::{
    async_lesson::VideoRepository,
    lesson::{LessonCompletionRepository, RecordCompletionError},
    live_lesson::LiveLessonSessionRepository,
};
use domain::{
    DomainError,
    async_lesson::{ReviewComment, Video},
    identity::{AccountId, Sport},
    lesson::LessonType,
};
use infra::{PgLessonCompletionRepository, PgLiveLessonSessionRepository, PgVideoRepository};
use sqlx::PgPool;
use support::at;

/// `student` が `coach` に送り、返却された動画。
async fn reviewed_video(pool: &PgPool, student: AccountId, coach: AccountId) -> Video {
    let videos = PgVideoRepository::new(pool.clone());
    let video = Video::submit(videos.next_video_id(), student, coach, Sport::Tennis, at(1));
    videos.insert(video.clone()).await.unwrap();
    let review_id = videos.next_review_id();
    videos
        .update(video.id(), |v| {
            v.return_review(review_id, coach, ReviewComment::new("よい").unwrap(), true, at(2))
        })
        .await
        .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn first_lesson_spans_live_and_async_completions(pool: PgPool) {
    let completions = PgLessonCompletionRepository::new(pool.clone());
    let session = support::ended_session(&pool, 11).await;
    let student = session.student_id();

    let id = completions.next_id();
    let live = completions
        .record(student, |history| session.complete(id, student, history, at(12)))
        .await
        .unwrap();
    assert!(live.is_first_lesson());

    let video = reviewed_video(&pool, student, session.coach_id()).await;
    let id = completions.next_id();
    let async_lesson = completions
        .record(student, |history| video.complete(id, student, history, at(13)))
        .await
        .unwrap();
    assert_eq!(async_lesson.lesson_type(), LessonType::Async);
    assert!(!async_lesson.is_first_lesson());

    // 同じセッションの 2 回目は拒否（読んだ履歴に元の記録が戻っている）
    let id = completions.next_id();
    let result = completions
        .record(student, |history| {
            assert_eq!(history, [live.clone(), async_lesson.clone()]);
            session.complete(id, student, history, at(14))
        })
        .await;
    assert!(
        matches!(
            result,
            Err(RecordCompletionError::Rejected(DomainError::LessonAlreadyCompleted))
        ),
        "{result:?}"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn concurrent_first_records_of_a_student_leave_one_first_lesson(pool: PgPool) {
    let completions = PgLessonCompletionRepository::new(pool.clone());
    let session = support::ended_session(&pool, 11).await;
    let student = session.student_id();
    let video = reviewed_video(&pool, student, session.coach_id()).await;
    // セッションの存在を確かめておく（外部キーの参照先）
    assert!(
        PgLiveLessonSessionRepository::new(pool.clone())
            .find(session.id())
            .await
            .unwrap()
            .is_some()
    );

    let (live_id, async_id) = (completions.next_id(), completions.next_id());
    let (live, async_lesson) = tokio::join!(
        completions.record(student, |history| session.complete(live_id, student, history, at(12))),
        completions.record(student, |history| video.complete(async_id, student, history, at(12))),
    );
    let firsts = [live.unwrap(), async_lesson.unwrap()]
        .iter()
        .filter(|c| c.is_first_lesson())
        .count();
    assert_eq!(firsts, 1);
}
