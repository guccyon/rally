//! `PgBookingRepository` の `CoachLeaveRepository`（`scheduling.coach_leaves`・`scheduling.coach_leave_bookings` と、お休みで
//! 変える予約と枠）の検証。期間の枠と予約の読み取り、お休みにする操作の保存（予約のキャンセル・キャンセルした人・枠の
//! 取り下げ・記録）と、断ったときに何も保存しないこと、キャンセルした予約からのお休みの記録の読み取り。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use std::collections::BTreeSet;

use application::scheduling::{BookingRepository, CoachLeaveRepository, SaveCoachLeaveError};
use domain::{
    identity::Role,
    scheduling::{
        BookingCanceller, BookingPeriod, BookingStatus, CoachLeave, CoachLeaveNote,
        CoachLeaveRequest, SchedulingError, SlotStatus,
    },
};
use infra::PgBookingRepository;
use sqlx::PgPool;
use support::{at, coach, confirmed_booking, open_slot, register, student};

/// `at(24)` から `at(24 * 4)` まで（3 日）
fn period() -> BookingPeriod {
    BookingPeriod::new(at(24), at(24 * 4)).unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn starting_a_leave_saves_the_cancellations_withdrawals_and_the_record(pool: PgPool) {
    let (student, coach) = (student(&pool).await, coach(&pool).await);
    let headquarters = register(&pool, Role::Headquarters, vec![]).await;
    let booked = open_slot(&pool, coach, 30, 31).await;
    let open = open_slot(&pool, coach, 40, 41).await;
    let outside = open_slot(&pool, coach, 200, 201).await;
    let booking = confirmed_booking(&pool, student, &booked).await;
    let kept = confirmed_booking(&pool, student, &outside).await;
    let bookings = PgBookingRepository::new(pool.clone());

    let (slots, read) = bookings
        .coach_slots_starting_in(coach, period())
        .await
        .unwrap();
    let mut slot_ids: Vec<_> = slots.iter().map(|s| s.id()).collect();
    slot_ids.sort();
    let mut expected_slots = vec![booked.id(), open.id()];
    expected_slots.sort();
    assert_eq!(slot_ids, expected_slots, "期間の外の枠は読まない");
    assert_eq!(
        read.iter().map(|b| b.id()).collect::<Vec<_>>(),
        vec![booking.id()]
    );

    let request = CoachLeaveRequest {
        coach_id: coach,
        period: period(),
        note: CoachLeaveNote::new("ご迷惑をおかけします。\n来週からは予約できます。").unwrap(),
        requested_by: headquarters,
        expected_booking_ids: BTreeSet::from([booking.id()]),
    };
    let id = bookings.next_coach_leave_id();
    let leave = bookings
        .start_coach_leave(coach, period(), |slots, bookings, existing| {
            CoachLeave::start(id, &request, at(2), slots, bookings, existing)
                .map(|(leave, _)| leave)
        })
        .await
        .unwrap();
    assert_eq!(leave.withdrawn_slot_count(), 2);

    let cancelled = bookings.find(booking.id()).await.unwrap().unwrap();
    assert_eq!(cancelled.status(), BookingStatus::Cancelled);
    assert_eq!(cancelled.cancelled_at(), Some(at(2)));
    assert_eq!(
        cancelled.cancelled_by(),
        Some(BookingCanceller::Headquarters)
    );
    assert_eq!(
        bookings.find(kept.id()).await.unwrap().unwrap().status(),
        BookingStatus::Confirmed
    );
    for slot in [&booked, &open] {
        assert_eq!(
            bookings
                .find_slot(slot.id())
                .await
                .unwrap()
                .unwrap()
                .status(),
            SlotStatus::Closed
        );
    }
    assert_eq!(
        bookings
            .find_slot(outside.id())
            .await
            .unwrap()
            .unwrap()
            .status(),
        SlotStatus::Booked
    );

    let read_back = bookings
        .coach_leaves_of_bookings(&BTreeSet::from([booking.id(), kept.id()]))
        .await
        .unwrap();
    assert_eq!(read_back, vec![leave.clone()], "記録は書いたとおりに読める");

    // 同じ期間の 2 回目は重なるお休みとして断り、何も保存しない
    let again_id = bookings.next_coach_leave_id();
    let mut again = request.clone();
    again.expected_booking_ids = BTreeSet::new();
    let error = bookings
        .start_coach_leave(coach, period(), |slots, bookings, existing| {
            CoachLeave::start(again_id, &again, at(3), slots, bookings, existing)
                .map(|(leave, _)| leave)
        })
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        SaveCoachLeaveError::Rejected(SchedulingError::CoachLeaveOverlaps)
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM scheduling.coach_leaves")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_rejected_leave_saves_nothing(pool: PgPool) {
    let (student, coach) = (student(&pool).await, coach(&pool).await);
    let headquarters = register(&pool, Role::Headquarters, vec![]).await;
    let booked = open_slot(&pool, coach, 30, 31).await;
    let booking = confirmed_booking(&pool, student, &booked).await;
    let bookings = PgBookingRepository::new(pool.clone());

    // 確かめた予約がない（確かめた後に入った予約）
    let request = CoachLeaveRequest {
        coach_id: coach,
        period: period(),
        note: CoachLeaveNote::default(),
        requested_by: headquarters,
        expected_booking_ids: BTreeSet::new(),
    };
    let id = bookings.next_coach_leave_id();
    let error = bookings
        .start_coach_leave(coach, period(), |slots, bookings, existing| {
            CoachLeave::start(id, &request, at(2), slots, bookings, existing)
                .map(|(leave, _)| leave)
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        SaveCoachLeaveError::Rejected(SchedulingError::CoachLeavePreviewOutdated)
    ));
    let read = bookings.find(booking.id()).await.unwrap().unwrap();
    assert_eq!(read.status(), BookingStatus::Confirmed);
    assert_eq!(read.cancelled_by(), None);
    assert_eq!(
        bookings
            .find_slot(booked.id())
            .await
            .unwrap()
            .unwrap()
            .status(),
        SlotStatus::Booked
    );
    assert!(
        bookings
            .coach_leaves_of_bookings(&BTreeSet::from([booking.id()]))
            .await
            .unwrap()
            .is_empty()
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn a_student_cancel_is_read_back_as_the_student(pool: PgPool) {
    let (student, coach) = (student(&pool).await, coach(&pool).await);
    let slot = open_slot(&pool, coach, 30, 31).await;
    let booking = confirmed_booking(&pool, student, &slot).await;
    let bookings = PgBookingRepository::new(pool.clone());
    bookings
        .update(booking.id(), |booking, slot| {
            booking.cancel(student, slot, at(2)).map(|_| ())
        })
        .await
        .unwrap();
    let value: Option<String> =
        sqlx::query_scalar("SELECT cancelled_by FROM scheduling.bookings WHERE booking_id = $1")
            .bind(application::scheduling::booking_id_uuid::to_uuid(
                booking.id(),
            ))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(value.as_deref(), Some("student"));
    // 列を足す前のキャンセル（NULL）も生徒として読む
    sqlx::query("UPDATE scheduling.bookings SET cancelled_by = NULL WHERE booking_id = $1")
        .bind(application::scheduling::booking_id_uuid::to_uuid(
            booking.id(),
        ))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        bookings
            .find(booking.id())
            .await
            .unwrap()
            .unwrap()
            .cancelled_by(),
        Some(BookingCanceller::Student)
    );
}
