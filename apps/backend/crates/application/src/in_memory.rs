//! ポートのインメモリ実装。テストと、infra の実装（Postgres・モックのトラッキング）を
//! 組み立てるまでのローカル起動に使う。プロセスが終われば消える。

use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

use domain::{
    DomainError,
    entitlement::Entitlement,
    identity::{Account, AccountId},
    scheduling::{Booking, BookingId, CoachCalendar, Slot, SlotId},
};

use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
    domain_event::{DomainEvent, DomainEventPublisher},
    entitlement::EntitlementRepository,
    identity::{AccountRepository, InsertAccountError, account_id_uuid},
    scheduling::{
        AddToCalendarError, BookingRepository, ReserveSlotError, SlotCriteria, SlotRepository,
        UpdateBookingError, booking_id_uuid, slot_id_uuid,
    },
};

/// [`AccountRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
#[derive(Debug, Clone, Default)]
pub struct InMemoryAccountRepository {
    accounts: Arc<Mutex<Vec<Account>>>,
}

impl InMemoryAccountRepository {
    /// 保存したアカウント（保存した順）。
    pub fn all(&self) -> Vec<Account> {
        self.accounts.lock().unwrap().clone()
    }
}

impl AccountRepository for InMemoryAccountRepository {
    fn next_id(&self) -> AccountId {
        account_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn insert(&self, account: Account) -> Result<(), InsertAccountError> {
        let mut accounts = self.accounts.lock().unwrap();
        let email = account.email().normalized();
        if accounts.iter().any(|a| a.email().normalized() == email) {
            return Err(InsertAccountError::EmailAlreadyRegistered);
        }
        accounts.push(account);
        Ok(())
    }
}

/// 発火したイベントを記録する [`AnalyticsSink`]。複製は同じ記録を共有する。
#[derive(Debug, Clone, Default)]
pub struct RecordingAnalyticsSink {
    events: Arc<Mutex<Vec<AnalyticsEvent>>>,
}

impl RecordingAnalyticsSink {
    /// 発火したイベント（発火した順）。
    pub fn events(&self) -> Vec<AnalyticsEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl AnalyticsSink for RecordingAnalyticsSink {
    async fn emit(&self, event: AnalyticsEvent) {
        self.events.lock().unwrap().push(event);
    }
}

/// 常に同じ時刻を返す [`Clock`]。
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub SystemTime);

impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

/// [`SlotRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
///
/// カレンダーの読み取りから保存までは 1 つのロックの中で行うので、同じコーチへの公開は直列になる。
#[derive(Debug, Clone, Default)]
pub struct InMemorySlotRepository {
    slots: Arc<Mutex<Vec<Slot>>>,
}

impl InMemorySlotRepository {
    /// 既存の枠を入れて作る（重なりは確かめない。テストの準備用）。
    pub fn with_slots(slots: impl IntoIterator<Item = Slot>) -> Self {
        Self {
            slots: Arc::new(Mutex::new(slots.into_iter().collect())),
        }
    }

    /// 保存した枠（保存した順）。
    pub fn all(&self) -> Vec<Slot> {
        self.slots.lock().unwrap().clone()
    }
}

impl SlotRepository for InMemorySlotRepository {
    fn next_id(&self) -> SlotId {
        slot_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn add_to_calendar<F>(&self, slot: Slot, add: F) -> Result<(), AddToCalendarError>
    where
        F: FnOnce(&mut CoachCalendar, Slot) -> Result<(), DomainError> + Send,
    {
        let mut slots = self.slots.lock().unwrap();
        let mut calendar = CoachCalendar::new(
            slot.coach_id(),
            slots
                .iter()
                .filter(|s| s.coach_id() == slot.coach_id())
                .cloned(),
        )
        .map_err(|e| AddToCalendarError::Unavailable(Unavailable::new(e)))?;
        add(&mut calendar, slot.clone()).map_err(AddToCalendarError::Rejected)?;
        slots.push(slot);
        Ok(())
    }

    async fn search(&self, criteria: &SlotCriteria) -> Result<Vec<Slot>, Unavailable> {
        let mut found: Vec<Slot> = self
            .slots
            .lock()
            .unwrap()
            .iter()
            .filter(|slot| criteria.matches(slot))
            .cloned()
            .collect();
        found.sort_by_key(|slot| (slot.starts_at(), slot.id()));
        found.truncate(criteria.limit);
        Ok(found)
    }
}

/// [`BookingRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
///
/// スロットは渡した [`InMemorySlotRepository`] と共有する（確定でスロットが埋まる）。読み取りから保存までは
/// スロット → 予約の順に 2 つのロックを取った中で行うので、予約の作成・確定とスロットの公開は直列になる。
#[derive(Debug, Clone)]
pub struct InMemoryBookingRepository {
    slots: InMemorySlotRepository,
    bookings: Arc<Mutex<Vec<Booking>>>,
}

impl InMemoryBookingRepository {
    pub fn new(slots: InMemorySlotRepository) -> Self {
        Self {
            slots,
            bookings: Arc::default(),
        }
    }

    /// 共有しているスロットの保存先。
    pub fn slots(&self) -> &InMemorySlotRepository {
        &self.slots
    }

    /// 保存した予約（保存した順）。
    pub fn all(&self) -> Vec<Booking> {
        self.bookings.lock().unwrap().clone()
    }
}

impl BookingRepository for InMemoryBookingRepository {
    fn next_id(&self) -> BookingId {
        booking_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn reserve<F>(
        &self,
        slot_id: SlotId,
        student_id: AccountId,
        reserve: F,
    ) -> Result<Booking, ReserveSlotError>
    where
        F: FnOnce(&Slot, &[Booking]) -> Result<Booking, DomainError> + Send,
    {
        let slots = self.slots.slots.lock().unwrap();
        let mut bookings = self.bookings.lock().unwrap();
        let slot = slots
            .iter()
            .find(|slot| slot.id() == slot_id)
            .ok_or(ReserveSlotError::SlotNotFound)?;
        let existing: Vec<Booking> = bookings
            .iter()
            .filter(|b| b.slot_id() == slot_id || b.student_id() == student_id)
            .cloned()
            .collect();
        let booking = reserve(slot, &existing).map_err(ReserveSlotError::Rejected)?;
        bookings.push(booking.clone());
        Ok(booking)
    }

    async fn update<F>(
        &self,
        booking_id: BookingId,
        update: F,
    ) -> Result<Booking, UpdateBookingError>
    where
        F: FnOnce(&mut Booking, &mut Slot) -> Result<(), DomainError> + Send,
    {
        let mut slots = self.slots.slots.lock().unwrap();
        let mut bookings = self.bookings.lock().unwrap();
        let booking = bookings
            .iter_mut()
            .find(|b| b.id() == booking_id)
            .ok_or(UpdateBookingError::BookingNotFound)?;
        let slot = slots
            .iter_mut()
            .find(|s| s.id() == booking.slot_id())
            .ok_or_else(|| {
                UpdateBookingError::Unavailable(Unavailable::new(
                    "予約のスロットが保存されていない",
                ))
            })?;
        // 失敗したときに書きかけを残さないよう、写しに適用してから書き戻す
        let (mut next_booking, mut next_slot) = (booking.clone(), slot.clone());
        update(&mut next_booking, &mut next_slot).map_err(UpdateBookingError::Rejected)?;
        *booking = next_booking.clone();
        *slot = next_slot;
        Ok(next_booking)
    }
}

/// 発行したドメインイベントを記録する [`DomainEventPublisher`]。複製は同じ記録を共有する。
#[derive(Debug, Clone, Default)]
pub struct RecordingDomainEventPublisher {
    events: Arc<Mutex<Vec<DomainEvent>>>,
}

impl RecordingDomainEventPublisher {
    /// 発行したイベント（発行した順）。
    pub fn events(&self) -> Vec<DomainEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl DomainEventPublisher for RecordingDomainEventPublisher {
    async fn publish(&self, event: DomainEvent) {
        self.events.lock().unwrap().push(event);
    }
}

/// [`EntitlementRepository`] のインメモリ実装。複製は同じ中身を共有する。
///
/// 受講権を作るユースケース（月額契約の開始）は T11 なので、当面はテストの準備で入れたものだけを返す。
#[derive(Debug, Clone, Default)]
pub struct InMemoryEntitlementRepository {
    entitlements: Arc<Mutex<Vec<Entitlement>>>,
}

impl InMemoryEntitlementRepository {
    /// 既存の受講権を入れて作る（生徒ごとに 1 つかは確かめない。テストの準備用）。
    pub fn with_entitlements(entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        Self {
            entitlements: Arc::new(Mutex::new(entitlements.into_iter().collect())),
        }
    }

    /// 保存した受講権（保存した順）。
    pub fn all(&self) -> Vec<Entitlement> {
        self.entitlements.lock().unwrap().clone()
    }
}

impl EntitlementRepository for InMemoryEntitlementRepository {
    async fn find_by_student(
        &self,
        student_id: AccountId,
    ) -> Result<Option<Entitlement>, Unavailable> {
        Ok(self
            .entitlements
            .lock()
            .unwrap()
            .iter()
            .find(|entitlement| entitlement.student_id() == student_id)
            .cloned())
    }
}
