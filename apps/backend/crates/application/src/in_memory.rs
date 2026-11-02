//! ポートのインメモリ実装。テストと、infra の実装（Postgres・モックのトラッキング）を
//! 組み立てるまでのローカル起動に使う。プロセスが終われば消える。

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use domain::{
    DomainError,
    async_lesson::{Video, VideoContentType, VideoId, VideoReviewId},
    catalog::{LessonMenu, LessonMenuId, LessonMenuPublicationChange},
    entitlement::{Entitlement, EntitlementError, EntitlementId, Subscription, SubscriptionId},
    identity::{Account, AccountId},
    lesson::{LessonCompletion, LessonCompletionId},
    live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
    matching::FavoriteCoach,
    messaging::{MessageId, MessageThread, MessageThreadId, MessagingError},
    notification::{
        Announcement, AnnouncementDraft, AnnouncementDraftId, AnnouncementId, Notification,
        NotificationId, NotificationKind, NotificationSettings, PushNotification,
    },
    profile::{CoachBio, DisplayName, DisplayNameQuery},
    review::{LessonReview, LessonReviewId, LessonReviewReply, LowRatedReviewHandling},
    scheduling::{
        Booking, BookingId, CoachAcceptance, CoachAcceptanceChange, CoachAcceptanceStatus,
        CoachCalendar, SchedulingError, Slot, SlotId,
    },
};

use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    async_lesson::{
        AiFeedbackGenerator, AiFeedbackRequest, UpdateVideoError, VideoRepository, VideoStorage,
        video_id_uuid, video_review_id_uuid,
    },
    catalog::{ChangeLessonMenuPublicationError, LessonMenuRepository, lesson_menu_id_uuid},
    clock::Clock,
    domain_event::{DomainEvent, DomainEventPublisher},
    entitlement::{
        EntitlementRepository, InsertEntitlementError, UpdateEntitlementError, entitlement_id_uuid,
        subscription_id_uuid,
    },
    identity::{AccountRepository, InsertAccountError, account_id_uuid},
    lesson::{LessonCompletionRepository, RecordCompletionError, lesson_completion_id_uuid},
    live_lesson::{
        InsertSessionError, LiveLessonSessionRepository, UpdateSessionError, VideoCallProvider,
        session_id_uuid,
    },
    matching::FavoriteCoachRepository,
    messaging::{
        MessageThreadRepository, UpdateMessageThreadError, message_id_uuid, message_thread_id_uuid,
    },
    notification::{
        AnnouncementDraftRepository, AnnouncementRepository, NotificationRepository,
        NotificationSettingsRepository, PushSender, announcement_draft_id_uuid,
        announcement_id_uuid, notification_id_uuid,
    },
    profile::CoachProfileRepository,
    review::{InsertReviewError, LessonReviewRepository, lesson_review_id_uuid},
    scheduling::{
        AddToCalendarError, BookingRepository, CoachAcceptanceRepository, ReserveSlotError,
        SlotCriteria, SlotRepository, UpdateBookingError, UpdateCoachAcceptanceError,
        UpdateSlotError, booking_id_uuid, slot_id_uuid,
    },
};

/// [`AccountRepository`] と profile の [`CoachProfileRepository`] のインメモリ実装（Postgres の実装と同じく、
/// コーチの自己紹介文もアカウントと同じ保存先に持つ）。ID は UUID v4。複製は同じ中身を共有する。
#[derive(Debug, Clone, Default)]
pub struct InMemoryAccountRepository {
    accounts: Arc<Mutex<Vec<Account>>>,
    /// コーチの自己紹介文。書いていないコーチは持たない（空として読む）
    coach_bios: Arc<Mutex<BTreeMap<AccountId, CoachBio>>>,
}

impl InMemoryAccountRepository {
    /// 保存したアカウント（保存した順）。
    pub fn all(&self) -> Vec<Account> {
        self.accounts.lock().unwrap().clone()
    }

    fn is_coach(&self, id: AccountId) -> bool {
        self.accounts
            .lock()
            .unwrap()
            .iter()
            .any(|a| a.id() == id && a.as_coach().is_some())
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

    async fn find(&self, id: AccountId) -> Result<Option<Account>, Unavailable> {
        Ok(self
            .accounts
            .lock()
            .unwrap()
            .iter()
            .find(|a| a.id() == id)
            .cloned())
    }

    async fn list_coaches(&self) -> Result<Vec<Account>, Unavailable> {
        let mut coaches: Vec<Account> = self
            .accounts
            .lock()
            .unwrap()
            .iter()
            .filter(|a| a.as_coach().is_some())
            .cloned()
            .collect();
        coaches.sort_by(|a, b| {
            (a.display_name().as_str(), a.id()).cmp(&(b.display_name().as_str(), b.id()))
        });
        Ok(coaches)
    }

    async fn list_students(
        &self,
        name: Option<&DisplayNameQuery>,
    ) -> Result<Vec<Account>, Unavailable> {
        let mut students: Vec<Account> = self
            .accounts
            .lock()
            .unwrap()
            .iter()
            .filter(|a| a.as_student().is_some())
            .filter(|a| name.is_none_or(|name| name.matches(a.display_name())))
            .cloned()
            .collect();
        students.sort_by(|a, b| {
            (std::cmp::Reverse(a.registered_at()), a.id())
                .cmp(&(std::cmp::Reverse(b.registered_at()), b.id()))
        });
        Ok(students)
    }

    async fn update_student_display_name(
        &self,
        student_id: AccountId,
        display_name: DisplayName,
    ) -> Result<Option<Account>, Unavailable> {
        let mut accounts = self.accounts.lock().unwrap();
        let Some(account) = accounts.iter_mut().find(|a| a.id() == student_id) else {
            return Ok(None);
        };
        if account.as_student().is_none() {
            return Ok(None);
        }
        *account = Account::with_role_attributes(
            account.id(),
            account.email().clone(),
            display_name,
            account.role_attributes().clone(),
            account.registered_at(),
        );
        Ok(Some(account.clone()))
    }
}

impl CoachProfileRepository for InMemoryAccountRepository {
    async fn find_coach_bio(&self, coach_id: AccountId) -> Result<Option<CoachBio>, Unavailable> {
        if !self.is_coach(coach_id) {
            return Ok(None);
        }
        let bios = self.coach_bios.lock().unwrap();
        Ok(Some(bios.get(&coach_id).cloned().unwrap_or_default()))
    }

    async fn update_coach_bio(
        &self,
        coach_id: AccountId,
        bio: CoachBio,
    ) -> Result<Option<CoachBio>, Unavailable> {
        if !self.is_coach(coach_id) {
            return Ok(None);
        }
        self.coach_bios
            .lock()
            .unwrap()
            .insert(coach_id, bio.clone());
        Ok(Some(bio))
    }

    async fn list_coach_bios(&self) -> Result<BTreeMap<AccountId, CoachBio>, Unavailable> {
        let coach_ids: Vec<AccountId> = self
            .accounts
            .lock()
            .unwrap()
            .iter()
            .filter(|a| a.as_coach().is_some())
            .map(|a| a.id())
            .collect();
        let bios = self.coach_bios.lock().unwrap();
        Ok(coach_ids
            .into_iter()
            .map(|id| (id, bios.get(&id).cloned().unwrap_or_default()))
            .collect())
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
/// カレンダーの読み取りから保存までは 1 つのロックの中で行うので、同じコーチへの公開は直列になる。枠の変更
/// （[`SlotRepository::update`]）も同じロックの中で行うので、[`InMemoryBookingRepository`] の予約と直列になる。
/// 検索から除く受付を止めているコーチは、[`Self::coach_acceptances`] の保存先と共有する。
#[derive(Debug, Clone, Default)]
pub struct InMemorySlotRepository {
    slots: Arc<Mutex<Vec<Slot>>>,
    acceptances: InMemoryCoachAcceptanceRepository,
}

impl InMemorySlotRepository {
    /// 既存の枠を入れて作る（重なりは確かめない。テストの準備用）。
    pub fn with_slots(slots: impl IntoIterator<Item = Slot>) -> Self {
        Self {
            slots: Arc::new(Mutex::new(slots.into_iter().collect())),
            acceptances: InMemoryCoachAcceptanceRepository::default(),
        }
    }

    /// 保存した枠（保存した順）。
    pub fn all(&self) -> Vec<Slot> {
        self.slots.lock().unwrap().clone()
    }

    /// 共有しているコーチの受付の保存先（複製は同じ中身を共有する）。受付を止めたコーチの枠は検索に出ず、
    /// [`InMemoryBookingRepository::find_coach_acceptance`] も同じ状態を読む。
    pub fn coach_acceptances(&self) -> InMemoryCoachAcceptanceRepository {
        self.acceptances.clone()
    }
}

impl SlotRepository for InMemorySlotRepository {
    fn next_id(&self) -> SlotId {
        slot_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn add_to_calendar<F>(&self, slot: Slot, add: F) -> Result<(), AddToCalendarError>
    where
        F: FnOnce(&mut CoachCalendar, Slot) -> Result<(), SchedulingError> + Send,
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

    async fn update<F>(&self, slot_id: SlotId, update: F) -> Result<Slot, UpdateSlotError>
    where
        F: FnOnce(&mut Slot) -> Result<(), SchedulingError> + Send,
    {
        let mut slots = self.slots.lock().unwrap();
        let slot = slots
            .iter_mut()
            .find(|slot| slot.id() == slot_id)
            .ok_or(UpdateSlotError::SlotNotFound)?;
        // 失敗したときに書きかけを残さないよう、写しに適用してから書き戻す
        let mut next = slot.clone();
        update(&mut next).map_err(UpdateSlotError::Rejected)?;
        *slot = next.clone();
        Ok(next)
    }

    async fn search(&self, criteria: &SlotCriteria) -> Result<Vec<Slot>, Unavailable> {
        let suspended = self.acceptances.suspended();
        let mut found: Vec<Slot> = self
            .slots
            .lock()
            .unwrap()
            .iter()
            .filter(|slot| criteria.matches(slot) && !suspended.contains(&slot.coach_id()))
            .cloned()
            .collect();
        found.sort_by_key(|slot| (slot.starts_at(), slot.id()));
        found.truncate(criteria.limit);
        Ok(found)
    }

    async fn slots_of_coach(&self, coach_id: AccountId) -> Result<Vec<Slot>, Unavailable> {
        Ok(self
            .slots
            .lock()
            .unwrap()
            .iter()
            .filter(|slot| slot.coach_id() == coach_id)
            .cloned()
            .collect())
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

    async fn find(&self, booking_id: BookingId) -> Result<Option<Booking>, Unavailable> {
        Ok(self
            .bookings
            .lock()
            .unwrap()
            .iter()
            .find(|b| b.id() == booking_id)
            .cloned())
    }

    async fn find_coach_acceptance(
        &self,
        coach_id: AccountId,
    ) -> Result<CoachAcceptance, Unavailable> {
        Ok(self.slots.acceptances.find(coach_id))
    }

    async fn find_slot(&self, slot_id: SlotId) -> Result<Option<Slot>, Unavailable> {
        Ok(self
            .slots
            .slots
            .lock()
            .unwrap()
            .iter()
            .find(|slot| slot.id() == slot_id)
            .cloned())
    }

    async fn reserve<F>(
        &self,
        slot_id: SlotId,
        student_id: AccountId,
        reserve: F,
    ) -> Result<Booking, ReserveSlotError>
    where
        F: FnOnce(&mut Slot, &[Booking]) -> Result<Booking, SchedulingError> + Send,
    {
        let mut slots = self.slots.slots.lock().unwrap();
        let mut bookings = self.bookings.lock().unwrap();
        let slot = slots
            .iter_mut()
            .find(|slot| slot.id() == slot_id)
            .ok_or(ReserveSlotError::SlotNotFound)?;
        let existing: Vec<Booking> = bookings
            .iter()
            .filter(|b| b.slot_id() == slot_id || b.student_id() == student_id)
            .cloned()
            .collect();
        // 失敗したときに書きかけを残さないよう、写しに適用してから書き戻す（席の数はスロットが持つ）
        let mut next_slot = slot.clone();
        let booking = reserve(&mut next_slot, &existing).map_err(ReserveSlotError::Rejected)?;
        *slot = next_slot;
        bookings.push(booking.clone());
        Ok(booking)
    }

    async fn update<F>(
        &self,
        booking_id: BookingId,
        update: F,
    ) -> Result<Booking, UpdateBookingError>
    where
        F: FnOnce(&mut Booking, &mut Slot) -> Result<(), SchedulingError> + Send,
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

    async fn slots_starting_between(
        &self,
        from: SystemTime,
        until: SystemTime,
    ) -> Result<(Vec<Slot>, Vec<Booking>), Unavailable> {
        let slots = self.slots.slots.lock().unwrap();
        let bookings = self.bookings.lock().unwrap();
        let found: Vec<Slot> = slots
            .iter()
            .filter(|s| from <= s.starts_at() && s.starts_at() < until)
            .cloned()
            .collect();
        let booked = bookings
            .iter()
            .filter(|b| found.iter().any(|s| s.id() == b.slot_id()))
            .cloned()
            .collect();
        Ok((found, booked))
    }

    async fn bookings_cancelled_between(
        &self,
        from: SystemTime,
        until: SystemTime,
    ) -> Result<Vec<Booking>, Unavailable> {
        Ok(self
            .bookings
            .lock()
            .unwrap()
            .iter()
            .filter(|b| b.cancelled_at().is_some_and(|at| from <= at && at < until))
            .cloned()
            .collect())
    }

    async fn bookings_of_student(
        &self,
        student_id: AccountId,
    ) -> Result<Vec<(Booking, Slot)>, Unavailable> {
        let slots = self.slots.slots.lock().unwrap();
        let bookings = self.bookings.lock().unwrap();
        bookings
            .iter()
            .filter(|b| b.student_id() == student_id)
            .map(|b| {
                let slot = slots
                    .iter()
                    .find(|s| s.id() == b.slot_id())
                    .ok_or_else(|| Unavailable::new("予約のスロットが保存されていない"))?;
                Ok((b.clone(), slot.clone()))
            })
            .collect()
    }

    async fn last_booked_at_of_students(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, SystemTime>, Unavailable> {
        let mut last = BTreeMap::new();
        for booking in self.bookings.lock().unwrap().iter() {
            if !student_ids.contains(&booking.student_id()) {
                continue;
            }
            last.entry(booking.student_id())
                .and_modify(|at: &mut SystemTime| *at = (*at).max(booking.created_at()))
                .or_insert(booking.created_at());
        }
        Ok(last)
    }

    async fn bookings_of_coach(
        &self,
        coach_id: AccountId,
    ) -> Result<Vec<(Booking, Slot)>, Unavailable> {
        let slots = self.slots.slots.lock().unwrap();
        let bookings = self.bookings.lock().unwrap();
        bookings
            .iter()
            .filter(|b| b.coach_id() == coach_id)
            .map(|b| {
                let slot = slots
                    .iter()
                    .find(|s| s.id() == b.slot_id())
                    .ok_or_else(|| Unavailable::new("予約のスロットが保存されていない"))?;
                Ok((b.clone(), slot.clone()))
            })
            .collect()
    }

    async fn bookings_of_lesson_menus_created_since(
        &self,
        lesson_menu_ids: &BTreeSet<LessonMenuId>,
        created_since: SystemTime,
    ) -> Result<Vec<Booking>, Unavailable> {
        Ok(self
            .bookings
            .lock()
            .unwrap()
            .iter()
            .filter(|b| {
                b.lesson_menu()
                    .is_some_and(|menu| lesson_menu_ids.contains(&menu.id()))
                    && b.created_at() >= created_since
            })
            .cloned()
            .collect())
    }
}

/// [`CoachAcceptanceRepository`] のインメモリ実装。受付を止めているコーチの ID（ないコーチは受付中）と、止めた・再開した
/// 記録を保存した順に持つ。複製は同じ中身を共有する。枠の検索と予約の作成が読めるよう、[`InMemorySlotRepository::coach_acceptances`] から
/// 取り出して使う（単独で作ると、どの枠の保存先とも共有しない）。
#[derive(Debug, Clone, Default)]
pub struct InMemoryCoachAcceptanceRepository {
    suspended: Arc<Mutex<BTreeSet<AccountId>>>,
    changes: Arc<Mutex<Vec<CoachAcceptanceChange>>>,
}

impl InMemoryCoachAcceptanceRepository {
    fn suspended(&self) -> BTreeSet<AccountId> {
        self.suspended.lock().unwrap().clone()
    }

    fn find(&self, coach_id: AccountId) -> CoachAcceptance {
        restore_acceptance(&self.suspended.lock().unwrap(), coach_id)
    }
}

/// 受付を止めているコーチの集合 `suspended` から、`coach_id` のコーチの受付を戻す。
fn restore_acceptance(suspended: &BTreeSet<AccountId>, coach_id: AccountId) -> CoachAcceptance {
    let status = if suspended.contains(&coach_id) {
        CoachAcceptanceStatus::Suspended
    } else {
        CoachAcceptanceStatus::Accepting
    };
    CoachAcceptance::restore(coach_id, status)
}

impl CoachAcceptanceRepository for InMemoryCoachAcceptanceRepository {
    async fn update<F>(
        &self,
        coach_id: AccountId,
        update: F,
    ) -> Result<CoachAcceptance, UpdateCoachAcceptanceError>
    where
        F: FnOnce(&mut CoachAcceptance) -> Result<CoachAcceptanceChange, SchedulingError> + Send,
    {
        let mut suspended = self.suspended.lock().unwrap();
        let mut acceptance = restore_acceptance(&suspended, coach_id);
        let change = update(&mut acceptance).map_err(UpdateCoachAcceptanceError::Rejected)?;
        match acceptance.status() {
            CoachAcceptanceStatus::Suspended => suspended.insert(coach_id),
            CoachAcceptanceStatus::Accepting => suspended.remove(&coach_id),
        };
        self.changes.lock().unwrap().push(change);
        Ok(acceptance)
    }

    async fn suspended_coaches(&self) -> Result<BTreeSet<AccountId>, Unavailable> {
        Ok(self.suspended())
    }

    async fn changes(
        &self,
        coach_id: AccountId,
    ) -> Result<Vec<CoachAcceptanceChange>, Unavailable> {
        // 保存した順の逆にしてから時刻で安定に並べるので、同じ時刻は後に保存した方が先
        let mut changes: Vec<_> = self
            .changes
            .lock()
            .unwrap()
            .iter()
            .rev()
            .filter(|change| change.coach_id() == coach_id)
            .cloned()
            .collect();
        changes.sort_by_key(|change| std::cmp::Reverse(change.changed_at()));
        Ok(changes)
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

/// [`EntitlementRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
///
/// 読み取りから保存までは 1 つのロックの中で行うので、同じ生徒の枠の消費は直列になる。
#[derive(Debug, Clone, Default)]
pub struct InMemoryEntitlementRepository {
    entitlements: Arc<Mutex<Vec<Entitlement>>>,
    subscriptions: Arc<Mutex<Vec<Subscription>>>,
}

impl InMemoryEntitlementRepository {
    /// 既存の受講権を入れて作る（生徒ごとに 1 つかは確かめない。テストの準備用）。
    pub fn with_entitlements(entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        Self {
            entitlements: Arc::new(Mutex::new(entitlements.into_iter().collect())),
            subscriptions: Arc::default(),
        }
    }

    /// 保存した受講権（保存した順）。
    pub fn all(&self) -> Vec<Entitlement> {
        self.entitlements.lock().unwrap().clone()
    }

    /// 保存した月額契約（保存した順）。
    pub fn subscriptions(&self) -> Vec<Subscription> {
        self.subscriptions.lock().unwrap().clone()
    }
}

impl EntitlementRepository for InMemoryEntitlementRepository {
    fn next_entitlement_id(&self) -> EntitlementId {
        entitlement_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    fn next_subscription_id(&self) -> SubscriptionId {
        subscription_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

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

    async fn insert_monthly(
        &self,
        subscription: Subscription,
        entitlement: Entitlement,
    ) -> Result<(), InsertEntitlementError> {
        // 受講権 → 月額契約の順にロックを取る（update_by_student と同じ順）
        let mut entitlements = self.entitlements.lock().unwrap();
        let mut subscriptions = self.subscriptions.lock().unwrap();
        if entitlements
            .iter()
            .any(|e| e.student_id() == entitlement.student_id())
        {
            return Err(InsertEntitlementError::AlreadyExists);
        }
        subscriptions.push(subscription);
        entitlements.push(entitlement);
        Ok(())
    }

    async fn update_by_student<F>(
        &self,
        student_id: AccountId,
        update: F,
    ) -> Result<Entitlement, UpdateEntitlementError>
    where
        F: FnOnce(&mut Entitlement) -> Result<(), EntitlementError> + Send,
    {
        let mut entitlements = self.entitlements.lock().unwrap();
        let entitlement = entitlements
            .iter_mut()
            .find(|e| e.student_id() == student_id)
            .ok_or(UpdateEntitlementError::NotFound)?;
        // 失敗したときに書きかけを残さないよう、写しに適用してから書き戻す
        let mut next = entitlement.clone();
        update(&mut next).map_err(UpdateEntitlementError::Rejected)?;
        *entitlement = next.clone();
        Ok(next)
    }
}

/// 外から進められる [`Clock`]（月をまたぐテスト用）。複製は同じ時刻を共有する。
#[derive(Debug, Clone)]
pub struct ManualClock(Arc<Mutex<SystemTime>>);

impl ManualClock {
    pub fn new(now: SystemTime) -> Self {
        Self(Arc::new(Mutex::new(now)))
    }

    /// 現在時刻を `now` にする。
    pub fn set(&self, now: SystemTime) {
        *self.0.lock().unwrap() = now;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self.0.lock().unwrap()
    }
}

/// [`LiveLessonSessionRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
///
/// 読み取りから保存までは 1 つのロックの中で行うので、同じセッションの終了・同じ予約の開始は直列になる。
#[derive(Debug, Clone, Default)]
pub struct InMemoryLiveLessonSessionRepository {
    sessions: Arc<Mutex<Vec<LiveLessonSession>>>,
}

impl InMemoryLiveLessonSessionRepository {
    /// 既存のセッションを入れて作る（予約ごとに 1 つかは確かめない。テストの準備用）。
    pub fn with_sessions(sessions: impl IntoIterator<Item = LiveLessonSession>) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(sessions.into_iter().collect())),
        }
    }

    /// 保存したセッション（保存した順）。
    pub fn all(&self) -> Vec<LiveLessonSession> {
        self.sessions.lock().unwrap().clone()
    }
}

impl LiveLessonSessionRepository for InMemoryLiveLessonSessionRepository {
    fn next_id(&self) -> SessionId {
        session_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn find(&self, session_id: SessionId) -> Result<Option<LiveLessonSession>, Unavailable> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.id() == session_id)
            .cloned())
    }

    async fn find_by_booking(
        &self,
        booking_id: BookingId,
    ) -> Result<Option<LiveLessonSession>, Unavailable> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.bookings().iter().any(|b| b.booking_id() == booking_id))
            .cloned())
    }

    async fn find_in_progress_started_at_or_before(
        &self,
        started_at_or_before: SystemTime,
    ) -> Result<Vec<LiveLessonSession>, Unavailable> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.ended_at().is_none() && s.started_at() <= started_at_or_before)
            .cloned()
            .collect())
    }

    async fn find_started_between(
        &self,
        starts_at: SystemTime,
        ends_at: SystemTime,
        coach_id: Option<AccountId>,
    ) -> Result<Vec<LiveLessonSession>, Unavailable> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|s| {
                starts_at <= s.started_at()
                    && s.started_at() < ends_at
                    && coach_id.is_none_or(|coach_id| s.coach_id() == coach_id)
            })
            .cloned()
            .collect())
    }

    async fn insert(&self, session: LiveLessonSession) -> Result<(), InsertSessionError> {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.iter().any(|s| {
            s.bookings().iter().any(|linked| {
                session
                    .bookings()
                    .iter()
                    .any(|b| b.booking_id() == linked.booking_id())
            })
        }) {
            return Err(InsertSessionError::BookingAlreadyHasSession);
        }
        sessions.push(session);
        Ok(())
    }

    async fn update<F>(
        &self,
        session_id: SessionId,
        update: F,
    ) -> Result<LiveLessonSession, UpdateSessionError>
    where
        F: FnOnce(&mut LiveLessonSession) -> Result<(), DomainError> + Send,
    {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .iter_mut()
            .find(|s| s.id() == session_id)
            .ok_or(UpdateSessionError::NotFound)?;
        // 失敗したときに書きかけを残さないよう、写しに適用してから書き戻す
        let mut next = session.clone();
        update(&mut next).map_err(UpdateSessionError::Rejected)?;
        *session = next.clone();
        Ok(next)
    }
}

/// 閉じるときに常に同じ接続品質を返す [`VideoCallProvider`]（テスト用）。開いているルームを記録し（同じセッションの
/// ルームは 1 つ）、開いていないルームを閉じようとすると障害を返す。トークンは `fixed-room.<セッション ID>.<参加者 ID>`。
/// 複製は同じ記録を共有する。
#[derive(Debug, Clone)]
pub struct FixedVideoCallProvider {
    quality: ConnectionQuality,
    rooms: Arc<Mutex<Vec<SessionId>>>,
}

impl FixedVideoCallProvider {
    pub fn new(quality: ConnectionQuality) -> Self {
        Self {
            quality,
            rooms: Arc::default(),
        }
    }

    /// 開いているルーム（作った順）。
    pub fn open_rooms(&self) -> Vec<SessionId> {
        self.rooms.lock().unwrap().clone()
    }
}

impl Default for FixedVideoCallProvider {
    /// 損失なし・ジッタ 10 ms・往復 50 ms・切断なし。
    fn default() -> Self {
        Self::new(ConnectionQuality::new(0.0, 10, 50, 0).expect("損失率 0 は百分率の範囲内"))
    }
}

impl VideoCallProvider for FixedVideoCallProvider {
    async fn create_room(
        &self,
        session_id: SessionId,
        participant: AccountId,
    ) -> Result<String, Unavailable> {
        let mut rooms = self.rooms.lock().unwrap();
        if !rooms.contains(&session_id) {
            rooms.push(session_id);
        }
        Ok(format!(
            "fixed-room.{}.{}",
            session_id_uuid::format(session_id),
            account_id_uuid::format(participant)
        ))
    }

    async fn close_room(&self, session_id: SessionId) -> Result<ConnectionQuality, Unavailable> {
        let mut rooms = self.rooms.lock().unwrap();
        let index = rooms
            .iter()
            .position(|room| *room == session_id)
            .ok_or_else(|| Unavailable::new("ルームが開いていない"))?;
        rooms.remove(index);
        Ok(self.quality)
    }
}

/// [`VideoRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
///
/// 読み取りから保存までは 1 つのロックの中で行うので、同じ動画の返却は直列になる。
#[derive(Debug, Clone, Default)]
pub struct InMemoryVideoRepository {
    videos: Arc<Mutex<Vec<Video>>>,
}

impl InMemoryVideoRepository {
    /// 既存の動画を入れて作る（テストの準備用）。
    pub fn with_videos(videos: impl IntoIterator<Item = Video>) -> Self {
        Self {
            videos: Arc::new(Mutex::new(videos.into_iter().collect())),
        }
    }

    /// 保存した動画（保存した順）。
    pub fn all(&self) -> Vec<Video> {
        self.videos.lock().unwrap().clone()
    }
}

impl VideoRepository for InMemoryVideoRepository {
    fn next_video_id(&self) -> VideoId {
        video_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    fn next_review_id(&self) -> VideoReviewId {
        video_review_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn find(&self, video_id: VideoId) -> Result<Option<Video>, Unavailable> {
        Ok(self
            .videos
            .lock()
            .unwrap()
            .iter()
            .find(|v| v.id() == video_id)
            .cloned())
    }

    async fn videos_of_coach(&self, coach_id: AccountId) -> Result<Vec<Video>, Unavailable> {
        Ok(self
            .videos
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.coach_id() == coach_id)
            .cloned()
            .collect())
    }

    async fn videos_of_student(&self, student_id: AccountId) -> Result<Vec<Video>, Unavailable> {
        Ok(self
            .videos
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.student_id() == student_id)
            .cloned()
            .collect())
    }

    async fn videos_awaiting_review(&self) -> Result<Vec<Video>, Unavailable> {
        Ok(self
            .videos
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.review().is_none())
            .cloned()
            .collect())
    }

    async fn videos_returned_between(
        &self,
        starts_at: SystemTime,
        ends_at: SystemTime,
    ) -> Result<Vec<Video>, Unavailable> {
        Ok(self
            .videos
            .lock()
            .unwrap()
            .iter()
            .filter(|v| {
                v.review()
                    .is_some_and(|r| starts_at <= r.returned_at() && r.returned_at() < ends_at)
            })
            .cloned()
            .collect())
    }

    async fn insert(&self, video: Video) -> Result<(), Unavailable> {
        self.videos.lock().unwrap().push(video);
        Ok(())
    }

    async fn update<F>(&self, video_id: VideoId, update: F) -> Result<Video, UpdateVideoError>
    where
        F: FnOnce(&mut Video) -> Result<(), DomainError> + Send,
    {
        let mut videos = self.videos.lock().unwrap();
        let video = videos
            .iter_mut()
            .find(|v| v.id() == video_id)
            .ok_or(UpdateVideoError::NotFound)?;
        // 失敗したときに書きかけを残さないよう、写しに適用してから書き戻す
        let mut next = video.clone();
        update(&mut next).map_err(UpdateVideoError::Rejected)?;
        *video = next.clone();
        Ok(next)
    }
}

/// 常に `https://fixed-storage.invalid/videos/<動画 ID>` を返す [`VideoStorage`]（テスト用）。
#[derive(Debug, Clone, Copy, Default)]
pub struct FixedVideoStorage;

impl VideoStorage for FixedVideoStorage {
    async fn issue_upload_url(
        &self,
        video_id: VideoId,
        _content_type: VideoContentType,
    ) -> Result<String, Unavailable> {
        Ok(format!(
            "https://fixed-storage.invalid/videos/{}",
            video_id_uuid::format(video_id)
        ))
    }
}

/// 常に [`FixedAiFeedbackGenerator::COMMENT`] を返す [`AiFeedbackGenerator`]（テスト用）。
#[derive(Debug, Clone, Copy, Default)]
pub struct FixedAiFeedbackGenerator;

impl FixedAiFeedbackGenerator {
    /// 返す一次フィードバックの文。
    pub const COMMENT: &'static str = "AI の一次フィードバック（テスト用の固定の文）";
}

impl AiFeedbackGenerator for FixedAiFeedbackGenerator {
    async fn generate(&self, _request: &AiFeedbackRequest) -> Result<String, Unavailable> {
        Ok(Self::COMMENT.to_string())
    }
}

/// [`LessonCompletionRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
///
/// 読み取りから保存までは 1 つのロックの中で行うので、同じ生徒の記録は直列になる（初回が 2 つにならない）。
#[derive(Debug, Clone, Default)]
pub struct InMemoryLessonCompletionRepository {
    completions: Arc<Mutex<Vec<LessonCompletion>>>,
}

impl InMemoryLessonCompletionRepository {
    /// 既存の完了記録を入れて作る（初回の判定や重複は確かめない。テストの準備用）。
    pub fn with_completions(completions: impl IntoIterator<Item = LessonCompletion>) -> Self {
        Self {
            completions: Arc::new(Mutex::new(completions.into_iter().collect())),
        }
    }

    /// 保存した完了記録（保存した順）。
    pub fn all(&self) -> Vec<LessonCompletion> {
        self.completions.lock().unwrap().clone()
    }
}

impl LessonCompletionRepository for InMemoryLessonCompletionRepository {
    fn next_id(&self) -> LessonCompletionId {
        lesson_completion_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn find(
        &self,
        lesson_id: LessonCompletionId,
    ) -> Result<Option<LessonCompletion>, Unavailable> {
        Ok(self
            .completions
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.id() == lesson_id)
            .cloned())
    }

    async fn completions_of_student(
        &self,
        student_id: AccountId,
    ) -> Result<Vec<LessonCompletion>, Unavailable> {
        Ok(self
            .completions
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.student_id() == student_id)
            .cloned()
            .collect())
    }

    async fn find_by_coach(
        &self,
        coach_id: AccountId,
        completed_since: SystemTime,
    ) -> Result<Vec<LessonCompletion>, Unavailable> {
        Ok(self
            .completions
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.coach_id() == coach_id && c.completed_at() >= completed_since)
            .cloned()
            .collect())
    }

    async fn record<F>(
        &self,
        student_id: AccountId,
        record: F,
    ) -> Result<LessonCompletion, RecordCompletionError>
    where
        F: FnOnce(&[LessonCompletion]) -> Result<LessonCompletion, DomainError> + Send,
    {
        let mut completions = self.completions.lock().unwrap();
        let history: Vec<LessonCompletion> = completions
            .iter()
            .filter(|c| c.student_id() == student_id)
            .cloned()
            .collect();
        let completion = record(&history).map_err(RecordCompletionError::Rejected)?;
        completions.push(completion.clone());
        Ok(completion)
    }
}

/// [`MessageThreadRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
///
/// 読み取りから保存までは 1 つのロックの中で行うので、同じスレッドへの送信・既読と、同じ生徒とコーチの
/// スレッドを開くことは直列になる。
#[derive(Debug, Clone, Default)]
pub struct InMemoryMessageThreadRepository {
    threads: Arc<Mutex<Vec<MessageThread>>>,
}

impl InMemoryMessageThreadRepository {
    /// 既存のスレッドを入れて作る（同じ生徒とコーチのスレッドが 1 つかは確かめない。テストの準備用）。
    pub fn with_threads(threads: impl IntoIterator<Item = MessageThread>) -> Self {
        Self {
            threads: Arc::new(Mutex::new(threads.into_iter().collect())),
        }
    }

    /// 保存したスレッド（開いた順）。
    pub fn all(&self) -> Vec<MessageThread> {
        self.threads.lock().unwrap().clone()
    }
}

impl MessageThreadRepository for InMemoryMessageThreadRepository {
    fn next_thread_id(&self) -> MessageThreadId {
        message_thread_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    fn next_message_id(&self) -> MessageId {
        message_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn open(&self, thread: MessageThread) -> Result<MessageThread, Unavailable> {
        let mut threads = self.threads.lock().unwrap();
        if let Some(existing) = threads
            .iter()
            .find(|t| t.student_id() == thread.student_id() && t.coach_id() == thread.coach_id())
        {
            return Ok(existing.clone());
        }
        threads.push(thread.clone());
        Ok(thread)
    }

    async fn find(&self, thread_id: MessageThreadId) -> Result<Option<MessageThread>, Unavailable> {
        Ok(self
            .threads
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id() == thread_id)
            .cloned())
    }

    async fn list_for(&self, account_id: AccountId) -> Result<Vec<MessageThread>, Unavailable> {
        Ok(self
            .threads
            .lock()
            .unwrap()
            .iter()
            .filter(|t| t.student_id() == account_id || t.coach_id() == account_id)
            .cloned()
            .collect())
    }

    async fn update<F>(
        &self,
        thread_id: MessageThreadId,
        update: F,
    ) -> Result<MessageThread, UpdateMessageThreadError>
    where
        F: FnOnce(&mut MessageThread) -> Result<(), MessagingError> + Send,
    {
        let mut threads = self.threads.lock().unwrap();
        let thread = threads
            .iter_mut()
            .find(|t| t.id() == thread_id)
            .ok_or(UpdateMessageThreadError::NotFound)?;
        // 失敗したときに書きかけを残さないよう、写しに適用してから書き戻す
        let mut next = thread.clone();
        update(&mut next).map_err(UpdateMessageThreadError::Rejected)?;
        *thread = next.clone();
        Ok(next)
    }
}

/// [`LessonReviewRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
///
/// 保存は 1 つのロックの中で同じレッスンの評価を確かめてから行うので、同じレッスンへの並行な評価は 1 つだけ入る。
#[derive(Debug, Clone, Default)]
pub struct InMemoryLessonReviewRepository {
    reviews: Arc<Mutex<Vec<LessonReview>>>,
}

impl InMemoryLessonReviewRepository {
    /// 既存の評価を入れて作る（レッスンごとに 1 つかは確かめない。テストの準備用）。
    pub fn with_reviews(reviews: impl IntoIterator<Item = LessonReview>) -> Self {
        Self {
            reviews: Arc::new(Mutex::new(reviews.into_iter().collect())),
        }
    }

    /// 保存した評価（保存した順）。
    pub fn all(&self) -> Vec<LessonReview> {
        self.reviews.lock().unwrap().clone()
    }
}

impl LessonReviewRepository for InMemoryLessonReviewRepository {
    fn next_id(&self) -> LessonReviewId {
        lesson_review_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn find_by_id(
        &self,
        review_id: LessonReviewId,
    ) -> Result<Option<LessonReview>, Unavailable> {
        Ok(self
            .reviews
            .lock()
            .unwrap()
            .iter()
            .find(|r| r.id() == review_id)
            .cloned())
    }

    async fn find_by_lesson(
        &self,
        lesson_id: LessonCompletionId,
    ) -> Result<Option<LessonReview>, Unavailable> {
        Ok(self
            .reviews
            .lock()
            .unwrap()
            .iter()
            .find(|r| r.lesson_id() == lesson_id)
            .cloned())
    }

    async fn insert(&self, review: LessonReview) -> Result<(), InsertReviewError> {
        let mut reviews = self.reviews.lock().unwrap();
        if reviews.iter().any(|r| r.lesson_id() == review.lesson_id()) {
            return Err(InsertReviewError::LessonAlreadyReviewed);
        }
        reviews.push(review);
        Ok(())
    }

    async fn save_reply(
        &self,
        review_id: LessonReviewId,
        reply: &LessonReviewReply,
    ) -> Result<(), Unavailable> {
        if let Some(review) = self
            .reviews
            .lock()
            .unwrap()
            .iter_mut()
            .find(|r| r.id() == review_id)
        {
            *review = review.clone().with_reply(Some(reply.clone()));
        }
        Ok(())
    }

    async fn save_handling(
        &self,
        review_id: LessonReviewId,
        handling: &LowRatedReviewHandling,
    ) -> Result<(), Unavailable> {
        if let Some(review) = self
            .reviews
            .lock()
            .unwrap()
            .iter_mut()
            .find(|r| r.id() == review_id && r.handling().is_none())
        {
            *review = review.clone().with_handling(Some(handling.clone()));
        }
        Ok(())
    }

    async fn find_by_coach(&self, coach_id: AccountId) -> Result<Vec<LessonReview>, Unavailable> {
        Ok(self
            .reviews
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.coach_id() == coach_id)
            .cloned()
            .collect())
    }

    async fn find_submitted_since(
        &self,
        since: SystemTime,
    ) -> Result<Vec<LessonReview>, Unavailable> {
        Ok(self
            .reviews
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.submitted_at() >= since)
            .cloned()
            .collect())
    }
}

/// [`LessonMenuRepository`] のインメモリ実装。ID は UUID v4。本部が非公開にした・公開に戻した記録を保存した順に持つ。
/// 複製は同じ中身を共有する。
#[derive(Debug, Clone, Default)]
pub struct InMemoryLessonMenuRepository {
    menus: Arc<Mutex<Vec<LessonMenu>>>,
    publication_changes: Arc<Mutex<Vec<LessonMenuPublicationChange>>>,
}

impl InMemoryLessonMenuRepository {
    /// 既存のメニューを入れて作る（テストの準備用）。
    pub fn with_menus(menus: impl IntoIterator<Item = LessonMenu>) -> Self {
        Self {
            menus: Arc::new(Mutex::new(menus.into_iter().collect())),
            publication_changes: Arc::default(),
        }
    }

    /// 保存したメニュー（保存した順）。
    pub fn all(&self) -> Vec<LessonMenu> {
        self.menus.lock().unwrap().clone()
    }
}

impl LessonMenuRepository for InMemoryLessonMenuRepository {
    fn next_id(&self) -> LessonMenuId {
        lesson_menu_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn insert(&self, menu: LessonMenu) -> Result<(), Unavailable> {
        self.menus.lock().unwrap().push(menu);
        Ok(())
    }

    async fn find(&self, id: LessonMenuId) -> Result<Option<LessonMenu>, Unavailable> {
        Ok(self
            .menus
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.id() == id)
            .cloned())
    }

    async fn update(&self, menu: LessonMenu) -> Result<(), Unavailable> {
        let mut menus = self.menus.lock().unwrap();
        if let Some(saved) = menus.iter_mut().find(|m| m.id() == menu.id()) {
            *saved = menu;
        }
        Ok(())
    }

    async fn find_by_coach(&self, coach_id: AccountId) -> Result<Vec<LessonMenu>, Unavailable> {
        Ok(self
            .menus
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.coach_id() == coach_id)
            .cloned()
            .collect())
    }

    async fn find_all(&self) -> Result<Vec<LessonMenu>, Unavailable> {
        Ok(self.all())
    }

    async fn change_publication<F>(
        &self,
        id: LessonMenuId,
        change: F,
    ) -> Result<LessonMenu, ChangeLessonMenuPublicationError>
    where
        F: FnOnce(&mut LessonMenu) -> Result<LessonMenuPublicationChange, DomainError> + Send,
    {
        let mut menus = self.menus.lock().unwrap();
        let saved = menus
            .iter_mut()
            .find(|m| m.id() == id)
            .ok_or(ChangeLessonMenuPublicationError::LessonMenuNotFound)?;
        let mut menu = saved.clone();
        let record = change(&mut menu).map_err(ChangeLessonMenuPublicationError::Invalid)?;
        *saved = menu.clone();
        self.publication_changes.lock().unwrap().push(record);
        Ok(menu)
    }

    async fn publication_changes(
        &self,
        id: LessonMenuId,
    ) -> Result<Vec<LessonMenuPublicationChange>, Unavailable> {
        // 保存した順の逆にしてから時刻で安定に並べるので、同じ時刻は後に保存した方が先
        let mut changes: Vec<_> = self
            .publication_changes
            .lock()
            .unwrap()
            .iter()
            .rev()
            .filter(|change| change.lesson_menu_id() == id)
            .cloned()
            .collect();
        changes.sort_by_key(|change| std::cmp::Reverse(change.changed_at()));
        Ok(changes)
    }
}

/// 送った通知を記録する [`PushSender`]（テスト用）。複製は同じ記録を共有する。
#[derive(Debug, Clone, Default)]
pub struct RecordingPushSender {
    sent: Arc<Mutex<Vec<PushNotification>>>,
}

impl RecordingPushSender {
    /// 送った通知（送った順）。
    pub fn sent(&self) -> Vec<PushNotification> {
        self.sent.lock().unwrap().clone()
    }
}

impl PushSender for RecordingPushSender {
    async fn send(&self, notification: PushNotification) -> Result<(), Unavailable> {
        self.sent.lock().unwrap().push(notification);
        Ok(())
    }
}

/// [`NotificationSettingsRepository`] のインメモリ実装。複製は同じ中身を共有する。
///
/// 読み取りから保存までは 1 つのロックの中で行うので、同じアカウントの変更は直列になる。
#[derive(Debug, Clone, Default)]
pub struct InMemoryNotificationSettingsRepository {
    settings: Arc<Mutex<Vec<NotificationSettings>>>,
}

impl InMemoryNotificationSettingsRepository {
    /// 既存の受け取り設定を入れて作る（アカウントごとに 1 つかは確かめない。テストの準備用）。
    pub fn with_settings(settings: impl IntoIterator<Item = NotificationSettings>) -> Self {
        Self {
            settings: Arc::new(Mutex::new(settings.into_iter().collect())),
        }
    }

    /// 保存した受け取り設定（最初に保存した順）。
    pub fn all(&self) -> Vec<NotificationSettings> {
        self.settings.lock().unwrap().clone()
    }
}

impl NotificationSettingsRepository for InMemoryNotificationSettingsRepository {
    async fn find(&self, account_id: AccountId) -> Result<NotificationSettings, Unavailable> {
        Ok(self
            .settings
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.account_id() == account_id)
            .cloned()
            .unwrap_or_else(|| NotificationSettings::new(account_id)))
    }

    async fn update<F>(
        &self,
        account_id: AccountId,
        update: F,
    ) -> Result<NotificationSettings, Unavailable>
    where
        F: FnOnce(&mut NotificationSettings) + Send,
    {
        let mut all = self.settings.lock().unwrap();
        let index = match all.iter().position(|s| s.account_id() == account_id) {
            Some(index) => index,
            None => {
                all.push(NotificationSettings::new(account_id));
                all.len() - 1
            }
        };
        update(&mut all[index]);
        Ok(all[index].clone())
    }
}

/// [`NotificationRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
#[derive(Debug, Clone, Default)]
pub struct InMemoryNotificationRepository {
    notifications: Arc<Mutex<Vec<Notification>>>,
}

impl InMemoryNotificationRepository {
    /// 既存のお知らせを入れて作る（同じプッシュ通知が 1 件かは確かめない。テストの準備用）。
    pub fn with_notifications(notifications: impl IntoIterator<Item = Notification>) -> Self {
        Self {
            notifications: Arc::new(Mutex::new(notifications.into_iter().collect())),
        }
    }

    /// 残したお知らせ（残した順）。
    pub fn all(&self) -> Vec<Notification> {
        self.notifications.lock().unwrap().clone()
    }
}

impl NotificationRepository for InMemoryNotificationRepository {
    fn next_id(&self) -> NotificationId {
        notification_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn insert(&self, notification: Notification) -> Result<(), Unavailable> {
        let mut notifications = self.notifications.lock().unwrap();
        if !notifications
            .iter()
            .any(|n| n.push() == notification.push())
        {
            notifications.push(notification);
        }
        Ok(())
    }

    async fn find_by_recipient(
        &self,
        recipient: AccountId,
    ) -> Result<Vec<Notification>, Unavailable> {
        let mut mine: Vec<Notification> = self
            .notifications
            .lock()
            .unwrap()
            .iter()
            .rev()
            .filter(|n| n.recipient() == recipient)
            .copied()
            .collect();
        // 後に残したものから並べてあるので、安定な並べ替えで同じ時刻は後に残したものが先になる
        mine.sort_by_key(|n| std::cmp::Reverse(n.created_at()));
        Ok(mine)
    }

    async fn find_announcements_by_recipient(
        &self,
        recipient: AccountId,
        limit: usize,
    ) -> Result<Vec<Notification>, Unavailable> {
        let mut announcements = self.find_by_recipient(recipient).await?;
        announcements.retain(|n| n.kind() == NotificationKind::Announcement);
        announcements.truncate(limit);
        Ok(announcements)
    }

    async fn mark_all_read(&self, recipient: AccountId, at: SystemTime) -> Result<(), Unavailable> {
        for notification in self.notifications.lock().unwrap().iter_mut() {
            if notification.recipient() == recipient {
                notification.mark_read(at);
            }
        }
        Ok(())
    }
}

/// [`AnnouncementRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
#[derive(Debug, Clone, Default)]
pub struct InMemoryAnnouncementRepository {
    announcements: Arc<Mutex<Vec<Announcement>>>,
}

impl InMemoryAnnouncementRepository {
    /// 既存の送った記録を入れて作る（テストの準備用）。
    pub fn with_announcements(announcements: impl IntoIterator<Item = Announcement>) -> Self {
        Self {
            announcements: Arc::new(Mutex::new(announcements.into_iter().collect())),
        }
    }

    /// 残した送った記録（残した順）。
    pub fn all(&self) -> Vec<Announcement> {
        self.announcements.lock().unwrap().clone()
    }
}

impl AnnouncementRepository for InMemoryAnnouncementRepository {
    fn next_id(&self) -> AnnouncementId {
        announcement_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn insert(&self, announcement: Announcement) -> Result<(), Unavailable> {
        self.announcements.lock().unwrap().push(announcement);
        Ok(())
    }

    async fn find(&self, id: AnnouncementId) -> Result<Option<Announcement>, Unavailable> {
        Ok(self.all().into_iter().find(|a| a.id() == id))
    }

    async fn update(&self, announcement: Announcement) -> Result<(), Unavailable> {
        let mut announcements = self.announcements.lock().unwrap();
        if let Some(saved) = announcements
            .iter_mut()
            .find(|a| a.id() == announcement.id() && !a.is_withdrawn())
        {
            *saved = saved
                .clone()
                .with_withdrawal(announcement.withdrawal().copied());
        }
        Ok(())
    }

    async fn find_all(&self) -> Result<Vec<Announcement>, Unavailable> {
        let mut all: Vec<Announcement> = self.all().into_iter().rev().collect();
        // 後に残したものから並べてあるので、安定な並べ替えで同じ時刻は後に残したものが先になる
        all.sort_by_key(|a| std::cmp::Reverse(a.sent_at()));
        Ok(all)
    }

    async fn find_by_ids(
        &self,
        ids: &BTreeSet<AnnouncementId>,
    ) -> Result<BTreeMap<AnnouncementId, Announcement>, Unavailable> {
        Ok(self
            .all()
            .into_iter()
            .filter(|a| ids.contains(&a.id()))
            .map(|a| (a.id(), a))
            .collect())
    }
}

/// [`AnnouncementDraftRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
#[derive(Debug, Clone, Default)]
pub struct InMemoryAnnouncementDraftRepository {
    drafts: Arc<Mutex<Vec<AnnouncementDraft>>>,
}

impl InMemoryAnnouncementDraftRepository {
    /// 既存の下書きを入れて作る（テストの準備用）。
    pub fn with_drafts(drafts: impl IntoIterator<Item = AnnouncementDraft>) -> Self {
        Self {
            drafts: Arc::new(Mutex::new(drafts.into_iter().collect())),
        }
    }

    /// 残っている下書き（作った順）。
    pub fn all(&self) -> Vec<AnnouncementDraft> {
        self.drafts.lock().unwrap().clone()
    }
}

impl AnnouncementDraftRepository for InMemoryAnnouncementDraftRepository {
    fn next_id(&self) -> AnnouncementDraftId {
        announcement_draft_id_uuid::from_uuid(uuid::Uuid::new_v4())
    }

    async fn insert(&self, draft: AnnouncementDraft) -> Result<(), Unavailable> {
        self.drafts.lock().unwrap().push(draft);
        Ok(())
    }

    async fn find(
        &self,
        id: AnnouncementDraftId,
    ) -> Result<Option<AnnouncementDraft>, Unavailable> {
        Ok(self.all().into_iter().find(|d| d.id() == id))
    }

    async fn update(&self, draft: AnnouncementDraft) -> Result<bool, Unavailable> {
        let mut drafts = self.drafts.lock().unwrap();
        let Some(saved) = drafts.iter_mut().find(|d| d.id() == draft.id()) else {
            return Ok(false);
        };
        // 作った本部のアカウントと作った時刻は保存してあるものを残す
        let mut revised = saved.clone();
        revised
            .revise(
                draft.title(),
                draft.body(),
                draft.sport(),
                draft.updated_at(),
            )
            .map_err(Unavailable::new)?;
        *saved = revised;
        Ok(true)
    }

    async fn delete(&self, id: AnnouncementDraftId) -> Result<bool, Unavailable> {
        let mut drafts = self.drafts.lock().unwrap();
        let before = drafts.len();
        drafts.retain(|d| d.id() != id);
        Ok(drafts.len() != before)
    }

    async fn find_all(&self) -> Result<Vec<AnnouncementDraft>, Unavailable> {
        let mut all: Vec<AnnouncementDraft> = self.all().into_iter().rev().collect();
        // 後に作ったものから並べてあるので、安定な並べ替えで同じ時刻は後に作ったものが先になる
        all.sort_by_key(|d| std::cmp::Reverse(d.updated_at()));
        Ok(all)
    }
}

/// [`FavoriteCoachRepository`] のインメモリ実装。複製は同じ中身を共有する。
#[derive(Debug, Clone, Default)]
pub struct InMemoryFavoriteCoachRepository {
    favorites: Arc<Mutex<Vec<FavoriteCoach>>>,
}

impl InMemoryFavoriteCoachRepository {
    /// 既存のお気に入りを入れて作る（同じ組が 1 つかは確かめない。テストの準備用）。
    pub fn with_favorites(favorites: impl IntoIterator<Item = FavoriteCoach>) -> Self {
        Self {
            favorites: Arc::new(Mutex::new(favorites.into_iter().collect())),
        }
    }
}

impl FavoriteCoachRepository for InMemoryFavoriteCoachRepository {
    async fn add(&self, favorite: FavoriteCoach) -> Result<(), Unavailable> {
        let mut favorites = self.favorites.lock().unwrap();
        if !favorites
            .iter()
            .any(|f| f.student_id() == favorite.student_id() && f.coach_id() == favorite.coach_id())
        {
            favorites.push(favorite);
        }
        Ok(())
    }

    async fn remove(&self, student_id: AccountId, coach_id: AccountId) -> Result<(), Unavailable> {
        self.favorites
            .lock()
            .unwrap()
            .retain(|f| !(f.student_id() == student_id && f.coach_id() == coach_id));
        Ok(())
    }

    async fn find_by_student(
        &self,
        student_id: AccountId,
    ) -> Result<Vec<FavoriteCoach>, Unavailable> {
        let mut mine: Vec<FavoriteCoach> = self
            .favorites
            .lock()
            .unwrap()
            .iter()
            .filter(|f| f.student_id() == student_id)
            .copied()
            .collect();
        mine.sort_by(|a, b| {
            b.favorited_at()
                .cmp(&a.favorited_at())
                .then_with(|| a.coach_id().cmp(&b.coach_id()))
        });
        Ok(mine)
    }
}
