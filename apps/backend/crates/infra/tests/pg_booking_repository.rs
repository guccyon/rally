//! `PgBookingRepository`（`scheduling.bookings` と予約したスロット）の検証。予約の作成・確定・キャンセルの
//! 往復、二重予約と体験予約の 1 回限りの拒否（並行な予約を含む）、期間に開始する枠とその予約の読み取り（予約枠の需給）、
//! 生徒の予約とそのスロットの読み取り（本人の予約の一覧）。
//!
//! Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して
//! `cargo test -p infra -- --include-ignored` で走らせる。

mod support;

use application::scheduling::{BookingRepository, ReserveSlotError};
use domain::{
    DomainError,
    identity::AccountId,
    scheduling::{Booking, BookingStatus, Slot, SlotStatus},
};
use infra::PgBookingRepository;
use sqlx::PgPool;
use support::at;

async fn reserve(
    bookings: &PgBookingRepository,
    student: AccountId,
    slot: &Slot,
    is_trial: bool,
) -> Result<Booking, ReserveSlotError> {
    let id = bookings.next_id();
    bookings
        .reserve(slot.id(), student, |slot, existing| {
            Booking::reserve(id, student, is_trial, slot, existing, at(1))
        })
        .await
}

async fn slot_status(pool: &PgPool, slot: &Slot) -> String {
    sqlx::query_scalar("SELECT status FROM scheduling.slots WHERE slot_id = $1")
        .bind(application::scheduling::slot_id_uuid::to_uuid(slot.id()))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn booking_round_trips_through_confirm_and_cancel(pool: PgPool) {
    let bookings = PgBookingRepository::new(pool.clone());
    let student = support::student(&pool).await;
    let slot = support::open_slot(&pool, support::coach(&pool).await, 30, 31).await;

    let booking = reserve(&bookings, student, &slot, true).await.unwrap();
    assert_eq!(
        bookings.find(booking.id()).await.unwrap(),
        Some(booking.clone())
    );
    assert_eq!(booking.starts_at(), at(30));
    assert_eq!(booking.coach_id(), slot.coach_id());

    let confirmed = bookings
        .update(booking.id(), |booking, slot| {
            booking.confirm(student, slot, at(2))
        })
        .await
        .unwrap();
    assert_eq!(confirmed.status(), BookingStatus::Confirmed);
    assert_eq!(confirmed.confirmed_at(), Some(at(2)));
    assert_eq!(bookings.find(booking.id()).await.unwrap(), Some(confirmed));
    assert_eq!(slot_status(&pool, &slot).await, "booked");

    let cancelled = bookings
        .update(booking.id(), |booking, slot| {
            booking.cancel(student, slot, at(3)).map(|_| ())
        })
        .await
        .unwrap();
    assert_eq!(cancelled.cancelled_at(), Some(at(3)));
    assert_eq!(bookings.find(booking.id()).await.unwrap(), Some(cancelled));
    assert_eq!(slot_status(&pool, &slot).await, "open");
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn rejected_update_saves_nothing(pool: PgPool) {
    let bookings = PgBookingRepository::new(pool.clone());
    let student = support::student(&pool).await;
    let slot = support::open_slot(&pool, support::coach(&pool).await, 30, 31).await;
    let booking = reserve(&bookings, student, &slot, false).await.unwrap();

    let other = support::student(&pool).await;
    let result = bookings
        .update(booking.id(), |booking, slot| {
            booking.confirm(other, slot, at(2))
        })
        .await;
    assert!(result.is_err(), "{result:?}");
    assert_eq!(bookings.find(booking.id()).await.unwrap(), Some(booking));
    assert_eq!(slot_status(&pool, &slot).await, "open");
    assert_eq!(slot.status(), SlotStatus::Open);
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn double_booking_and_second_trial_are_rejected(pool: PgPool) {
    let bookings = PgBookingRepository::new(pool.clone());
    let coach = support::coach(&pool).await;
    let (first, second) = (support::student(&pool).await, support::student(&pool).await);
    let slot = support::open_slot(&pool, coach, 30, 31).await;
    reserve(&bookings, first, &slot, true).await.unwrap();

    let result = reserve(&bookings, second, &slot, false).await;
    assert!(
        matches!(
            result,
            Err(ReserveSlotError::Rejected(DomainError::SlotAlreadyBooked))
        ),
        "{result:?}"
    );
    let other_slot = support::open_slot(&pool, coach, 40, 41).await;
    let result = reserve(&bookings, first, &other_slot, true).await;
    assert!(
        matches!(
            result,
            Err(ReserveSlotError::Rejected(DomainError::TrialAlreadyUsed))
        ),
        "{result:?}"
    );
    let missing = support::open_slot(&pool, coach, 50, 51).await;
    sqlx::query("DELETE FROM scheduling.slots WHERE slot_id = $1")
        .bind(application::scheduling::slot_id_uuid::to_uuid(missing.id()))
        .execute(&pool)
        .await
        .unwrap();
    let result = reserve(&bookings, first, &missing, false).await;
    assert!(
        matches!(result, Err(ReserveSlotError::SlotNotFound)),
        "{result:?}"
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn concurrent_reservations_of_a_slot_save_only_one(pool: PgPool) {
    let bookings = PgBookingRepository::new(pool.clone());
    let slot = support::open_slot(&pool, support::coach(&pool).await, 30, 31).await;
    let mut students = Vec::new();
    for _ in 0..8 {
        students.push(support::student(&pool).await);
    }

    let results = join_all(
        students
            .into_iter()
            .map(|student| {
                let (bookings, slot) = (bookings.clone(), slot.clone());
                tokio::spawn(async move { reserve(&bookings, student, &slot, false).await })
            })
            .collect(),
    )
    .await;
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "{results:?}"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM scheduling.bookings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

async fn join_all<T: std::fmt::Debug>(handles: Vec<tokio::task::JoinHandle<T>>) -> Vec<T> {
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    results
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn slots_starting_between_reads_slots_of_any_status_and_their_bookings(pool: PgPool) {
    let bookings = PgBookingRepository::new(pool.clone());
    let coach = support::coach(&pool).await;
    let (first, second) = (support::student(&pool).await, support::student(&pool).await);
    let before = support::open_slot(&pool, coach, 9, 10).await;
    let start = support::open_slot(&pool, coach, 10, 11).await;
    let closed = support::open_slot(&pool, coach, 20, 21).await;
    let end = support::open_slot(&pool, coach, 30, 31).await;
    let cancelled = reserve(&bookings, first, &start, false).await.unwrap();
    bookings
        .update(cancelled.id(), |booking, slot| {
            booking.cancel(first, slot, at(2)).map(|_| ())
        })
        .await
        .unwrap();
    let confirmed = support::confirmed_booking(&pool, second, &start).await;
    reserve(&bookings, first, &before, false).await.unwrap();
    reserve(&bookings, second, &end, false).await.unwrap();
    sqlx::query("UPDATE scheduling.slots SET status = 'closed' WHERE slot_id = $1")
        .bind(application::scheduling::slot_id_uuid::to_uuid(closed.id()))
        .execute(&pool)
        .await
        .unwrap();

    // 始まりは含み、終わりは含まない
    let (mut slots, mut found) = bookings
        .slots_starting_between(at(10), at(30))
        .await
        .unwrap();
    slots.sort_by_key(Slot::starts_at);
    found.sort_by_key(|b| b.cancelled_at().is_none());
    let slot_ids: Vec<_> = slots.iter().map(|s| (s.id(), s.status())).collect();
    assert_eq!(
        slot_ids,
        [
            (start.id(), SlotStatus::Booked),
            (closed.id(), SlotStatus::Closed)
        ]
    );
    let booking_ids: Vec<_> = found.iter().map(|b| (b.id(), b.status())).collect();
    assert_eq!(
        booking_ids,
        [
            (cancelled.id(), BookingStatus::Cancelled),
            (confirmed.id(), BookingStatus::Confirmed)
        ]
    );
}

#[ignore = "Postgres が要る（DATABASE_URL）"]
#[sqlx::test(migrator = "infra::MIGRATOR")]
async fn bookings_of_student_reads_own_bookings_of_any_status_with_their_slots(pool: PgPool) {
    let bookings = PgBookingRepository::new(pool.clone());
    let coach = support::coach(&pool).await;
    let (student, other) = (support::student(&pool).await, support::student(&pool).await);
    let early = support::open_slot(&pool, coach, 10, 11).await;
    let late = support::open_slot(&pool, coach, 30, 31).await;
    let others = support::open_slot(&pool, coach, 40, 41).await;
    let cancelled = reserve(&bookings, student, &early, false).await.unwrap();
    let cancelled = bookings
        .update(cancelled.id(), |booking, slot| {
            booking.cancel(student, slot, at(2)).map(|_| ())
        })
        .await
        .unwrap();
    let confirmed = support::confirmed_booking(&pool, student, &late).await;
    reserve(&bookings, other, &others, false).await.unwrap();

    let mut found = bookings.bookings_of_student(student).await.unwrap();
    found.sort_by_key(|(_, slot)| slot.starts_at());
    let early = Slot::restore(
        early.id(),
        coach,
        early.sport(),
        early.starts_at(),
        early.ends_at(),
        SlotStatus::Open,
    )
    .unwrap();
    let late = Slot::restore(
        late.id(),
        coach,
        late.sport(),
        late.starts_at(),
        late.ends_at(),
        SlotStatus::Booked,
    )
    .unwrap();
    assert_eq!(found, [(cancelled, early), (confirmed.clone(), late)]);

    // ユースケースを通すと、スロットの開始時刻の新しい順
    let list = application::scheduling::ListMyBookings::new(
        bookings.clone(),
        application::identity::IdentityCoachIntroductionLookup::new(
            infra::PgAccountRepository::new(pool.clone()),
        ),
    )
    .execute(student)
    .await
    .unwrap();
    let ids: Vec<_> = list.items.iter().map(|(b, _)| b.status()).collect();
    assert_eq!(ids, [BookingStatus::Confirmed, BookingStatus::Cancelled]);
    assert_eq!(list.items[0].0, confirmed);
    assert_eq!(
        list.coaches.get(&coach).map(|c| c.display_name.as_str()),
        Some("テスト")
    );

    let nobody = support::student(&pool).await;
    assert!(
        bookings
            .bookings_of_student(nobody)
            .await
            .unwrap()
            .is_empty()
    );
}
