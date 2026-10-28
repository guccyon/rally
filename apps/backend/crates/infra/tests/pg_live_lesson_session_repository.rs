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
    let session = LiveLessonSession::start(
        sessions.next_id(),
        booking.student_id(),
        &booking,
        &[],
        None,
        at(10),
    )
    .unwrap();
    sessions.insert(session.clone()).await.unwrap();
    assert_eq!(
        sessions.find(session.id()).await.unwrap(),
        Some(session.clone())
    );
    assert_eq!(
        sessions.find_by_booking(booking.id()).await.unwrap(),
        Some(session.clone())
    );

    let ended = sessions
        .update(session.id(), |s| {
            s.end(booking.coach_id(), at(11), support::quality())
        })
        .await
        .unwrap();
    assert_eq!(ended.status(), SessionStatus::Ended);
    assert_eq!(ended.quality(), Some(support::quality()));
    assert_eq!(sessions.find(session.id()).await.unwrap(), Some(ended));

    // 終了は 1 回。拒否されたら何も変えない
    let result = sessions
        .update(session.id(), |s| {
            s.end(booking.coach_id(), at(12), support::quality())
        })
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
        sessions
            .find(session.id())
            .await
            .unwrap()
            .unwrap()
            .ended_at(),
        Some(at(11))
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_booking_has_at_most_one_session(pool: PgPool) {
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
    let booking = support::new_confirmed_booking(&pool).await;
    let start = || {
        LiveLessonSession::start(
            sessions.next_id(),
            booking.student_id(),
            &booking,
            &[],
            None,
            at(10),
        )
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_group_session_round_trips_with_its_linked_bookings(pool: PgPool) {
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
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
        &bookings[1],
        &bookings,
        None,
        at(10),
    )
    .unwrap();
    assert_eq!(session.bookings().len(), 3);
    sessions.insert(session.clone()).await.unwrap();

    // どの予約からも同じセッションを読める（先頭は開始した予約）
    assert_eq!(
        sessions.find(session.id()).await.unwrap(),
        Some(session.clone())
    );
    for booking in &bookings {
        assert_eq!(
            sessions.find_by_booking(booking.id()).await.unwrap(),
            Some(session.clone())
        );
    }
    let ended = sessions
        .update(session.id(), |s| s.end(coach, at(11), support::quality()))
        .await
        .unwrap();
    assert_eq!(ended.bookings(), session.bookings());

    // 結び付いた予約のどれかで別のセッションを始めても、保存先が拒む（何も保存しない）
    let other =
        LiveLessonSession::start(sessions.next_id(), coach, &bookings[2], &[], None, at(10))
            .unwrap();
    let result = sessions.insert(other.clone()).await;
    assert!(
        matches!(result, Err(InsertSessionError::BookingAlreadyHasSession)),
        "{result:?}"
    );
    assert_eq!(sessions.find(other.id()).await.unwrap(), None);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_session_without_linked_rows_reads_as_its_starting_booking(pool: PgPool) {
    // D41 より前の版が始めたセッション（session_bookings に行がない）
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
    let booking = support::new_confirmed_booking(&pool).await;
    let session = LiveLessonSession::start(
        sessions.next_id(),
        booking.student_id(),
        &booking,
        &[],
        None,
        at(10),
    )
    .unwrap();
    sessions.insert(session.clone()).await.unwrap();
    sqlx::query("DELETE FROM live_lesson.session_bookings")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sessions.find_by_booking(booking.id()).await.unwrap(),
        Some(session)
    );
}
