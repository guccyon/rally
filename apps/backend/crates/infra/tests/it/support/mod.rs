//! Postgres の保存先の統合テストの共通部品: アカウント・スロット・予約・セッション・動画を、保存先の実装を
//! 通して用意する（外部キーの参照先）。テストごとに使う部品が違うので、使わない部品の警告は出さない。
#![allow(dead_code)]

use std::time::{Duration, SystemTime};

use application::{
    identity::{RegisterAccount, RegisterAccountCommand},
    in_memory::{FixedClock, RecordingAnalyticsSink},
    live_lesson::LiveLessonSessionRepository,
    scheduling::{BookingRepository, SlotRepository},
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    live_lesson::{ConnectionQuality, LiveLessonSession},
    scheduling::{Booking, Slot},
};
use infra::{
    PgAccountRepository, PgBookingRepository, PgLiveLessonSessionRepository, PgSlotRepository,
};
use sqlx::PgPool;

/// 2026-11-01T00:00:00Z
const BASE: Duration = Duration::from_secs(1_793_491_200);

pub fn at(hour: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + BASE + Duration::from_secs(hour * 3600)
}

pub async fn register(pool: &PgPool, role: Role, sports: Vec<Sport>) -> AccountId {
    register_named(pool, role, sports, "テスト").await
}

/// 表示名 `display_name` のアカウント。
pub async fn register_named(
    pool: &PgPool,
    role: Role,
    sports: Vec<Sport>,
    display_name: &str,
) -> AccountId {
    RegisterAccount::new(
        PgAccountRepository::new(pool.clone()),
        RecordingAnalyticsSink::default(),
        FixedClock(at(0)),
    )
    .execute(RegisterAccountCommand {
        email: format!("{}@example.com", uuid::Uuid::new_v4()),
        display_name: display_name.to_string(),
        role,
        sports,
    })
    .await
    .unwrap()
    .id()
}

pub async fn student(pool: &PgPool) -> AccountId {
    register(pool, Role::Student, vec![Sport::Tennis]).await
}

pub async fn coach(pool: &PgPool) -> AccountId {
    register(pool, Role::Coach, vec![Sport::Tennis]).await
}

/// `coach` の公開中の枠（`from` 時から `to` 時まで。定員 1）。
pub async fn open_slot(pool: &PgPool, coach: AccountId, from: u64, to: u64) -> Slot {
    open_slot_with_capacity(pool, coach, from, to, 1).await
}

/// `coach` の定員 `capacity` の公開中の枠（`from` 時から `to` 時まで）。
pub async fn open_slot_with_capacity(
    pool: &PgPool,
    coach: AccountId,
    from: u64,
    to: u64,
    capacity: i64,
) -> Slot {
    let slots = PgSlotRepository::new(pool.clone());
    let slot = Slot::publish(
        slots.next_id(),
        coach,
        Sport::Tennis,
        at(from),
        at(to),
        domain::scheduling::SlotCapacity::new(capacity).unwrap(),
    )
    .unwrap();
    slots
        .add_to_calendar(slot.clone(), |calendar, slot| calendar.publish(slot))
        .await
        .unwrap();
    slot
}

/// `student` が `slot` を予約して確定した予約。
pub async fn confirmed_booking(pool: &PgPool, student: AccountId, slot: &Slot) -> Booking {
    let bookings = PgBookingRepository::new(pool.clone());
    let id = bookings.next_id();
    let booking = bookings
        .reserve(slot.id(), student, |slot, existing| {
            Booking::reserve(id, student, false, slot, existing, at(0))
        })
        .await
        .unwrap();
    bookings
        .update(booking.id(), |booking, slot| {
            booking.confirm(student, slot, at(0))
        })
        .await
        .unwrap()
}

/// 新しい生徒とコーチの、確定した予約。
pub async fn new_confirmed_booking(pool: &PgPool) -> Booking {
    let (student, coach) = (student(pool).await, coach(pool).await);
    let slot = open_slot(pool, coach, 10, 11).await;
    confirmed_booking(pool, student, &slot).await
}

pub fn quality() -> ConnectionQuality {
    ConnectionQuality::new(3.5, 60, 250, 1).unwrap()
}

/// 新しい予約の、`ended_at` 時に終了したセッション。
pub async fn ended_session(pool: &PgPool, ended_at: u64) -> LiveLessonSession {
    let booking = new_confirmed_booking(pool).await;
    let sessions = PgLiveLessonSessionRepository::new(pool.clone());
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
    sessions
        .update(session.id(), |session| {
            session.end(booking.coach_id(), at(ended_at), quality())
        })
        .await
        .unwrap()
}
