//! `PgBookingRepository`（`scheduling.bookings` と予約したスロット）の検証。予約の作成・確定・キャンセルの
//! 往復、二重予約と体験予約の 1 回限りの拒否（並行な予約を含む）。
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
    assert_eq!(bookings.find(booking.id()).await.unwrap(), Some(booking.clone()));
    assert_eq!(booking.starts_at(), at(30));
    assert_eq!(booking.coach_id(), slot.coach_id());

    let confirmed = bookings
        .update(booking.id(), |booking, slot| booking.confirm(student, slot, at(2)))
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
        .update(booking.id(), |booking, slot| booking.confirm(other, slot, at(2)))
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
        matches!(result, Err(ReserveSlotError::Rejected(DomainError::SlotAlreadyBooked))),
        "{result:?}"
    );
    let other_slot = support::open_slot(&pool, coach, 40, 41).await;
    let result = reserve(&bookings, first, &other_slot, true).await;
    assert!(
        matches!(result, Err(ReserveSlotError::Rejected(DomainError::TrialAlreadyUsed))),
        "{result:?}"
    );
    let missing = support::open_slot(&pool, coach, 50, 51).await;
    sqlx::query("DELETE FROM scheduling.slots WHERE slot_id = $1")
        .bind(application::scheduling::slot_id_uuid::to_uuid(missing.id()))
        .execute(&pool)
        .await
        .unwrap();
    let result = reserve(&bookings, first, &missing, false).await;
    assert!(matches!(result, Err(ReserveSlotError::SlotNotFound)), "{result:?}");
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
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1, "{results:?}");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM scheduling.bookings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

async fn join_all<T: std::fmt::Debug>(
    handles: Vec<tokio::task::JoinHandle<T>>,
) -> Vec<T> {
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    results
}
