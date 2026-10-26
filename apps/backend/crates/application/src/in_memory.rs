//! ポートのインメモリ実装。テストと、infra の実装（Postgres・モックのトラッキング）を
//! 組み立てるまでのローカル起動に使う。プロセスが終われば消える。

use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

use domain::{
    DomainError,
    identity::{Account, AccountId},
    scheduling::{CoachCalendar, Slot, SlotId},
};

use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
    identity::{AccountRepository, InsertAccountError, account_id_uuid},
    scheduling::{AddToCalendarError, SlotCriteria, SlotRepository, slot_id_uuid},
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
