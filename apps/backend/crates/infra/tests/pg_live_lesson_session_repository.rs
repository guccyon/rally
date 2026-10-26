//! `PgLiveLessonSessionRepository`（`live_lesson.sessions`）の検証。開始・終了の往復と、同じ予約の 2 つ目の拒否。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::live_lesson::{InsertSessionError, LiveLessonSessionRepository};
use domain::{
    DomainError,
    live_lesson::{LiveLessonSession, SessionStatus},
};
use infra::PgLiveLessonSessionRepository;
use sqlx::PgPool;
use support::at;

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn session_round_trips_through_start_and_end(pool: PgPool) {
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
    let booking = support::new_confirmed_booking(&pool).await;
    let session =
        LiveLessonSession::start(sessions.next_id(), booking.student_id(), &booking, None, at(10))
            .unwrap();
    sessions.insert(session.clone()).await.unwrap();
    assert_eq!(sessions.find(session.id()).await.unwrap(), Some(session.clone()));
    assert_eq!(
        sessions.find_by_booking(booking.id()).await.unwrap(),
        Some(session.clone())
    );

    let ended = sessions
        .update(session.id(), |s| s.end(booking.coach_id(), at(11), support::quality()))
        .await
        .unwrap();
    assert_eq!(ended.status(), SessionStatus::Ended);
    assert_eq!(ended.quality(), Some(support::quality()));
    assert_eq!(sessions.find(session.id()).await.unwrap(), Some(ended));

    // 終了は 1 回。拒否されたら何も変えない
    let result = sessions
        .update(session.id(), |s| s.end(booking.coach_id(), at(12), support::quality()))
        .await;
    assert!(
        matches!(
            result,
            Err(application::live_lesson::UpdateSessionError::Rejected(
                DomainError::LiveLessonSessionAlreadyEnded
            ))
        ),
        "{result:?}"
    );
    assert_eq!(
        sessions.find(session.id()).await.unwrap().unwrap().ended_at(),
        Some(at(11))
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_booking_has_at_most_one_session(pool: PgPool) {
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
    let booking = support::new_confirmed_booking(&pool).await;
    let start = || {
        LiveLessonSession::start(sessions.next_id(), booking.student_id(), &booking, None, at(10))
            .unwrap()
    };
    sessions.insert(start()).await.unwrap();
    let result = sessions.insert(start()).await;
    assert!(
        matches!(result, Err(InsertSessionError::BookingAlreadyHasSession)),
        "{result:?}"
    );
    assert_eq!(sessions.find(start().id()).await.unwrap(), None);
}
