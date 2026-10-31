//! `PgLessonCompletionRepository`（`live_lesson.lesson_completions`・`async_lesson.lesson_completions`）の検証。
//! 同期・非同期をまたいだ初回の判定、同じレッスンの 2 回目の拒否、並行な記録で初回が 1 件になること、生徒ごとに
//! 同期・非同期の両方を読めること。
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
    catalog::Sport,
    identity::AccountId,
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
            v.return_review(
                review_id,
                coach,
                ReviewComment::new("よい").unwrap(),
                true,
                at(2),
            )
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
        .record(student, |history| {
            session.complete(id, student, history, at(12))
        })
        .await
        .unwrap();
    assert!(live.is_first_lesson());

    let video = reviewed_video(&pool, student, session.coach_id()).await;
    let id = completions.next_id();
    let async_lesson = completions
        .record(student, |history| {
            video.complete(id, student, history, at(13))
        })
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
            Err(RecordCompletionError::Rejected(
                DomainError::LessonAlreadyCompleted
            ))
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
        completions.record(student, |history| session.complete(
            live_id,
            student,
            history,
            at(12)
        )),
        completions.record(student, |history| video.complete(
            async_id,
            student,
            history,
            at(12)
        )),
    );
    let firsts = [live.unwrap(), async_lesson.unwrap()]
        .iter()
        .filter(|c| c.is_first_lesson())
        .count();
    assert_eq!(firsts, 1);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn finds_live_and_async_completions_by_their_lesson_id(pool: PgPool) {
    let completions = PgLessonCompletionRepository::new(pool.clone());
    let session = support::ended_session(&pool, 11).await;
    let student = session.student_id();
    let id = completions.next_id();
    let live = completions
        .record(student, |history| {
            session.complete(id, student, history, at(12))
        })
        .await
        .unwrap();
    let video = reviewed_video(&pool, student, session.coach_id()).await;
    let id = completions.next_id();
    let async_lesson = completions
        .record(student, |history| {
            video.complete(id, student, history, at(13))
        })
        .await
        .unwrap();

    assert_eq!(completions.find(live.id()).await.unwrap(), Some(live));
    assert_eq!(
        completions.find(async_lesson.id()).await.unwrap(),
        Some(async_lesson)
    );
    assert_eq!(completions.find(completions.next_id()).await.unwrap(), None);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn reads_every_live_and_async_completion_of_a_student(pool: PgPool) {
    let completions = PgLessonCompletionRepository::new(pool.clone());
    let session = support::ended_session(&pool, 11).await;
    let student = session.student_id();
    let id = completions.next_id();
    let live = completions
        .record(student, |history| {
            session.complete(id, student, history, at(12))
        })
        .await
        .unwrap();
    let video = reviewed_video(&pool, student, session.coach_id()).await;
    let id = completions.next_id();
    let async_lesson = completions
        .record(student, |history| {
            video.complete(id, student, history, at(13))
        })
        .await
        .unwrap();
    // 別の生徒の完了したレッスンは読まない
    let other_student = support::student(&pool).await;
    let others_video = reviewed_video(&pool, other_student, session.coach_id()).await;
    let id = completions.next_id();
    completions
        .record(other_student, |history| {
            others_video.complete(id, other_student, history, at(14))
        })
        .await
        .unwrap();

    let mut found = completions.completions_of_student(student).await.unwrap();
    found.sort_by_key(|completion| completion.completed_at());
    assert_eq!(found, [live, async_lesson]);
    assert!(
        completions
            .completions_of_student(support::student(&pool).await)
            .await
            .unwrap()
            .is_empty()
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_group_session_is_completed_once_per_linked_student(pool: PgPool) {
    use domain::live_lesson::LiveLessonSession;

    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
    let completions = PgLessonCompletionRepository::new(pool.clone());
    let coach = support::coach(&pool).await;
    let slot = support::open_slot_with_capacity(&pool, coach, 10, 11, 3).await;
    let mut bookings = Vec::new();
    for _ in 0..3 {
        let student = support::student(&pool).await;
        bookings.push(support::confirmed_booking(&pool, student, &slot).await);
    }
    let session = LiveLessonSession::start(
        sessions.next_id(),
        coach,
        &bookings[0],
        &bookings,
        None,
        at(10),
    )
    .unwrap();
    sessions.insert(session.clone()).await.unwrap();
    let session = sessions
        .update(session.id(), |s| s.end(coach, at(11), support::quality()))
        .await
        .unwrap();

    // 同じセッションでも生徒ごとに 1 回ずつ入り、それぞれが生徒の初回
    for booking in &bookings {
        let student = booking.student_id();
        let id = completions.next_id();
        let completion = completions
            .record(student, |history| {
                session.complete_for(id, coach, student, history, at(12))
            })
            .await
            .unwrap();
        assert_eq!(completion.student_id(), student);
        assert!(completion.is_first_lesson());
        assert_eq!(completions.find(id).await.unwrap(), Some(completion));
    }
    let student = bookings[1].student_id();
    let id = completions.next_id();
    let result = completions
        .record(student, |history| {
            session.complete_for(id, student, student, history, at(13))
        })
        .await;
    assert!(
        matches!(
            result,
            Err(RecordCompletionError::Rejected(
                DomainError::LessonAlreadyCompleted
            ))
        ),
        "{result:?}"
    );
}
