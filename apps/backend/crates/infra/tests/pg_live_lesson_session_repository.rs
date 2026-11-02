//! `PgLiveLessonSessionRepository`（`live_lesson.sessions`）の検証。開始・終了の往復と、同じ予約の 2 つ目の拒否と、
//! 本部が見る、終わっていないまま時間の過ぎたセッションの読み取り（通話中で、ある時刻までに開始したもの）と、期間とコーチ・生徒で
//! 見るセッションの記録の読み取り（ある期間に開始したもの。コーチ・生徒で絞る）。
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

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn reads_in_progress_sessions_started_at_or_before_the_time(pool: PgPool) {
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
    let mut started = Vec::new();
    for hour in [5, 2, 3, 4] {
        let booking = support::new_confirmed_booking(&pool).await;
        let session = LiveLessonSession::start(
            sessions.next_id(),
            booking.student_id(),
            &booking,
            &[],
            None,
            at(hour),
        )
        .unwrap();
        sessions.insert(session.clone()).await.unwrap();
        started.push(session);
    }
    // 4 時に開始したものは終了した
    let ended = sessions
        .update(started[3].id(), |s| {
            s.end(s.coach_id(), at(6), support::quality())
        })
        .await
        .unwrap();
    assert_eq!(ended.status(), SessionStatus::Ended);
    // 終了したセッション（support の 10 時開始・11 時終了）も混ぜる
    support::ended_session(&pool, 11).await;

    // 4 時（含む）までに開始した通話中のもの: 2 時・3 時（4 時は終了済み、5 時は後）
    let found = sessions
        .find_in_progress_started_at_or_before(at(4))
        .await
        .unwrap();
    let mut found_ids: Vec<_> = found.iter().map(LiveLessonSession::id).collect();
    found_ids.sort();
    let mut expected = vec![started[1].id(), started[2].id()];
    expected.sort();
    assert_eq!(found_ids, expected);
    assert!(found.contains(&started[1]), "結び付いた予約ごと読む");

    // 境界の時刻ちょうどに開始したものは含む
    let found = sessions
        .find_in_progress_started_at_or_before(at(5))
        .await
        .unwrap();
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(
        sessions
            .find_in_progress_started_at_or_before(at(1))
            .await
            .unwrap()
            .is_empty()
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn reads_sessions_started_in_the_period_ended_or_not(pool: PgPool) {
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
    let mut started = Vec::new();
    for hour in [1, 2, 3, 4] {
        let booking = support::new_confirmed_booking(&pool).await;
        let session = LiveLessonSession::start(
            sessions.next_id(),
            booking.student_id(),
            &booking,
            &[],
            None,
            at(hour),
        )
        .unwrap();
        sessions.insert(session.clone()).await.unwrap();
        started.push(session);
    }
    // 2 時に開始したものは終了した
    let ended = sessions
        .update(started[1].id(), |s| {
            s.end(s.coach_id(), at(5), support::quality())
        })
        .await
        .unwrap();

    // 2 時（含む）から 4 時（含まない）までに開始したもの: 終了した 2 時と通話中の 3 時
    let found = sessions
        .find_started_between(at(2), at(4), None, None)
        .await
        .unwrap();
    let mut found_ids: Vec<_> = found.iter().map(LiveLessonSession::id).collect();
    found_ids.sort();
    let mut expected = vec![started[1].id(), started[2].id()];
    expected.sort();
    assert_eq!(found_ids, expected);
    assert!(found.contains(&ended), "終了の時刻と接続品質ごと読む");
    assert!(found.contains(&started[2]), "結び付いた予約ごと読む");

    // コーチで絞る（new_confirmed_booking はセッションごとに別のコーチ）
    let found = sessions
        .find_started_between(at(0), at(10), Some(started[2].coach_id()), None)
        .await
        .unwrap();
    assert_eq!(found, [started[2].clone()]);
    assert!(
        sessions
            .find_started_between(at(5), at(10), None, None)
            .await
            .unwrap()
            .is_empty()
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn reads_sessions_started_in_the_period_by_student_including_group_lessons(pool: PgPool) {
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
    // マンツーマンの通話（3 時）
    let one_on_one = support::new_confirmed_booking(&pool).await;
    let alone = LiveLessonSession::start(
        sessions.next_id(),
        one_on_one.student_id(),
        &one_on_one,
        &[],
        None,
        at(3),
    )
    .unwrap();
    sessions.insert(alone.clone()).await.unwrap();
    // 2 人のグループレッスン（10 時）。通話を始めたのは 1 人目の予約
    let coach = support::coach(&pool).await;
    let slot = support::open_slot_with_capacity(&pool, coach, 10, 11, 2).await;
    let mut bookings = Vec::new();
    for _ in 0..2 {
        let student = support::student(&pool).await;
        bookings.push(support::confirmed_booking(&pool, student, &slot).await);
    }
    let group = LiveLessonSession::start(
        sessions.next_id(),
        coach,
        &bookings[0],
        &bookings,
        None,
        at(10),
    )
    .unwrap();
    sessions.insert(group.clone()).await.unwrap();

    let find =
        |coach_id, student_id| sessions.find_started_between(at(0), at(12), coach_id, student_id);
    assert_eq!(
        find(None, Some(one_on_one.student_id())).await.unwrap(),
        std::slice::from_ref(&alone)
    );
    // 開始した予約の生徒でも、結び付いただけの予約の生徒でもグループレッスンのセッションが出る
    for booking in &bookings {
        assert_eq!(
            find(None, Some(booking.student_id())).await.unwrap(),
            std::slice::from_ref(&group)
        );
    }
    // コーチと組み合わさる。生徒でない ID は空
    assert!(
        find(Some(one_on_one.coach_id()), Some(bookings[1].student_id()))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(find(None, Some(coach)).await.unwrap().is_empty());
    // 期間と組み合わさる
    assert!(
        sessions
            .find_started_between(at(4), at(12), None, Some(one_on_one.student_id()))
            .await
            .unwrap()
            .is_empty()
    );
}
