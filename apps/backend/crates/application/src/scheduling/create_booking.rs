use std::collections::BTreeSet;

use domain::{
    catalog::LessonMenu,
    identity::AccountId,
    scheduling::{Booking, SchedulingError, SlotId},
};

use super::{BookingRepository, LessonMenuLookup, ReserveSlotError};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
};

/// 予約の作成の入力。`student_id` は認証した生徒のアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBookingCommand {
    pub student_id: AccountId,
    pub slot_id: SlotId,
    /// 体験予約か
    pub is_trial: bool,
}

#[derive(Debug)]
pub enum CreateBookingError {
    /// スロットがない
    SlotNotFound,
    /// 予約がドメインの不変条件を満たさない（公開中でない、満席、同じ生徒の 2 回目、2 回目の体験予約）
    Invalid(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 予約の作成のユースケース（体験予約を含む。student-app の `createBooking`）。
///
/// スロットとその予約・生徒の予約を読み、domain の [`Booking::reserve`] で不変条件（公開中のスロット
/// だけ、有効な予約はスロットの定員まで、同じ生徒は同じスロットに 1 つ、体験予約は初回だけ）を確かめて、
/// 確定前（`pending`）の予約を保存する。スロットはまだ埋めない（確定した予約が定員に達すると埋まる。
/// [`super::ConfirmBooking`]）。受講権も消費しない（ビデオ通話の暦月の枠は生徒ごとの予約の確定で数える）。
/// 予約には、スロットに結びついたレッスンメニューを catalog に問い合わせて（[`LessonMenuLookup`]）、予約した時点の
/// ID・名前・1 回の時間を写して残す（[`Booking::with_lesson_menu`]。公開中でないメニューはメニューなしの予約にし、
/// 予約は断らない）。
/// 保存したら行動イベント `booking.created` を 1 回発火する。拒否・保存の失敗では発火しない。
pub struct CreateBooking<B, M, A, C> {
    bookings: B,
    lesson_menus: M,
    analytics: A,
    clock: C,
}

impl<B, M, A, C> CreateBooking<B, M, A, C>
where
    B: BookingRepository,
    M: LessonMenuLookup,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(bookings: B, lesson_menus: M, analytics: A, clock: C) -> Self {
        Self {
            bookings,
            lesson_menus,
            analytics,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CreateBookingCommand,
    ) -> Result<Booking, CreateBookingError> {
        let id = self.bookings.next_id();
        let now = self.clock.now();
        let CreateBookingCommand {
            student_id,
            slot_id,
            is_trial,
        } = command;

        let lesson_menu = self
            .slot_lesson_menu(slot_id)
            .await
            .map_err(CreateBookingError::Unavailable)?;
        let booking = self
            .bookings
            .reserve(slot_id, student_id, |slot, existing| {
                Booking::reserve(id, student_id, is_trial, slot, existing, now)
                    .map(|booking| booking.with_lesson_menu(slot, lesson_menu.as_ref()))
            })
            .await
            .map_err(|error| match error {
                ReserveSlotError::SlotNotFound => CreateBookingError::SlotNotFound,
                ReserveSlotError::Rejected(error) => CreateBookingError::Invalid(error),
                ReserveSlotError::Unavailable(error) => CreateBookingError::Unavailable(error),
            })?;

        self.analytics
            .emit(AnalyticsEvent::BookingCreated {
                student_id: booking.student_id(),
                coach_id: booking.coach_id(),
                booking_id: booking.id(),
                slot_id: booking.slot_id(),
                is_trial: booking.is_trial(),
                occurred_at: now,
            })
            .await;

        Ok(booking)
    }

    /// スロットに結びついたレッスンメニュー（公開中かを問わない。写すかの判定は domain）。スロットがない・メニューが
    /// ない・catalog に引けないなら `None`（ないスロットは続く予約で [`CreateBookingError::SlotNotFound`] になる）。
    async fn slot_lesson_menu(&self, slot_id: SlotId) -> Result<Option<LessonMenu>, Unavailable> {
        let Some(lesson_menu_id) = self
            .bookings
            .find_slot(slot_id)
            .await?
            .and_then(|slot| slot.lesson_menu_id())
        else {
            return Ok(None);
        };
        Ok(self
            .lesson_menus
            .find_lesson_menus(&BTreeSet::from([lesson_menu_id]))
            .await?
            .remove(&lesson_menu_id))
    }

    pub fn bookings(&self) -> &B {
        &self.bookings
    }

    pub fn analytics(&self) -> &A {
        &self.analytics
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        catalog::{
            LessonDuration, LessonFormat, LessonMenuContent, LessonMenuDescription, LessonMenuId,
            LessonMenuName,
        },
        scheduling::{
            BookingId, BookingLessonMenu, BookingStatus, Slot, SlotCapacity, SlotSeats, SlotStatus,
        },
    };

    use super::*;
    use crate::{
        catalog::{CatalogLessonMenuLookup, LessonMenuRepository},
        in_memory::{
            FixedClock, InMemoryBookingRepository, InMemoryLessonMenuRepository,
            InMemorySlotRepository, RecordingAnalyticsSink,
        },
        scheduling::UpdateBookingError,
    };

    type UseCase = CreateBooking<
        InMemoryBookingRepository,
        CatalogLessonMenuLookup<InMemoryLessonMenuRepository>,
        RecordingAnalyticsSink,
        FixedClock,
    >;

    const NOW: SystemTime = SystemTime::UNIX_EPOCH;
    const COACH: AccountId = AccountId::from_u128(7);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);

    fn slot(id: u128, status: SlotStatus) -> Slot {
        // 予約済みの枠は確定した予約 1 件で埋まっている
        let confirmed = u32::from(status == SlotStatus::Booked);
        let starts_at = NOW + Duration::from_secs(id as u64 * 3600);
        Slot::restore(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            starts_at,
            starts_at + Duration::from_secs(3600),
            status,
            domain::scheduling::SlotSeats::restore(
                domain::scheduling::SlotCapacity::ONE,
                0,
                confirmed,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn group_slot(id: u128, capacity: i64) -> Slot {
        let open = slot(id, SlotStatus::Open);
        Slot::restore(
            open.id(),
            COACH,
            Sport::Tennis,
            open.starts_at(),
            open.ends_at(),
            SlotStatus::Open,
            SlotSeats::vacant(SlotCapacity::new(capacity).unwrap()),
        )
        .unwrap()
    }

    /// 1 回 60 分の同期のレッスンメニュー（コーチ 7・テニス）。
    fn lesson_menu(id: u128, name: &str, is_published: bool) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            COACH,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Live(LessonDuration::from_minutes(60).unwrap()),
                name: LessonMenuName::new(name).unwrap(),
                description: LessonMenuDescription::new("").unwrap(),
            },
            is_published,
            NOW,
        )
    }

    /// 公開中の枠 `id` を、公開した時点のメニュー `menu` に結びつける（結びつけた後にメニューが直されたことは
    /// 保存先のメニューで表す）。
    fn slot_with_menu(id: u128, menu: u128) -> Slot {
        let open = slot(id, SlotStatus::Open);
        Slot::publish(
            open.id(),
            COACH,
            Sport::Tennis,
            open.starts_at(),
            open.ends_at(),
            SlotCapacity::ONE,
        )
        .unwrap()
        .with_lesson_menu(&lesson_menu(menu, "公開した時点", true))
        .unwrap()
    }

    /// スロット 10・11・14 は公開中、12 は予約で埋まり、13 は閉じている。15 は定員 3 の公開中の枠。
    /// 16 は公開中のメニュー 50、17 は非公開に直したメニュー 51、18 は catalog にないメニュー 52 に結びついた公開中の枠。
    fn use_case() -> UseCase {
        use_case_and_menus().0
    }

    /// [`use_case`] と、その catalog のメニューの保存先（複製は同じ中身を共有する）。
    fn use_case_and_menus() -> (UseCase, InMemoryLessonMenuRepository) {
        let slots = InMemorySlotRepository::with_slots([
            slot(10, SlotStatus::Open),
            slot(11, SlotStatus::Open),
            slot(12, SlotStatus::Booked),
            slot(13, SlotStatus::Closed),
            slot(14, SlotStatus::Open),
            group_slot(15, 3),
            slot_with_menu(16, 50),
            slot_with_menu(17, 51),
            slot_with_menu(18, 52),
        ]);
        let menus = InMemoryLessonMenuRepository::with_menus([
            lesson_menu(50, "フォア改善", true),
            lesson_menu(51, "非公開に直した", false),
        ]);
        let use_case = CreateBooking::new(
            InMemoryBookingRepository::new(slots),
            CatalogLessonMenuLookup::new(menus.clone()),
            RecordingAnalyticsSink::default(),
            FixedClock(NOW),
        );
        (use_case, menus)
    }

    fn command(student_id: AccountId, slot: u128, is_trial: bool) -> CreateBookingCommand {
        CreateBookingCommand {
            student_id,
            slot_id: SlotId::from_u128(slot),
            is_trial,
        }
    }

    fn assert_invalid(result: Result<Booking, CreateBookingError>, expected: SchedulingError) {
        assert!(
            matches!(&result, Err(CreateBookingError::Invalid(error)) if *error == expected),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn creates_a_pending_booking_and_emits_booking_created_once() {
        let use_case = use_case();

        let booking = use_case.execute(command(STUDENT, 10, true)).await.unwrap();

        assert_eq!(booking.status(), BookingStatus::Pending);
        assert_eq!(booking.coach_id(), COACH);
        assert_eq!(booking.lesson_menu(), None, "メニューのない枠");
        assert_eq!(use_case.bookings().all(), vec![booking.clone()]);
        assert_eq!(
            use_case.bookings().slots().all()[0].status(),
            SlotStatus::Open,
            "作成ではスロットを埋めない"
        );
        let events = use_case.analytics().events();
        assert_eq!(
            events,
            vec![AnalyticsEvent::BookingCreated {
                student_id: STUDENT,
                coach_id: COACH,
                booking_id: booking.id(),
                slot_id: SlotId::from_u128(10),
                is_trial: true,
                occurred_at: NOW,
            }]
        );
        assert_eq!(events[0].name(), "booking.created");
    }

    #[tokio::test]
    async fn booking_copies_the_published_lesson_menu_of_the_slot_at_the_time_of_booking() {
        let (use_case, menus) = use_case_and_menus();

        let booking = use_case.execute(command(STUDENT, 16, false)).await.unwrap();

        let expected = BookingLessonMenu::copy_of(&lesson_menu(50, "フォア改善", true));
        assert_eq!(booking.lesson_menu(), Some(&expected), "今のメニューを写す");
        assert_eq!(use_case.bookings().all(), vec![booking.clone()]);

        // 予約した後にメニューを直しても、予約の写しは予約した時点のまま
        menus
            .update(lesson_menu(50, "直した名前", true))
            .await
            .unwrap();
        let stored = use_case.bookings().find(booking.id()).await.unwrap();
        assert_eq!(stored.unwrap().lesson_menu(), Some(&expected));
        assert_eq!(use_case.analytics().events().len(), 1);
    }

    #[tokio::test]
    async fn lesson_menu_that_is_not_published_or_missing_is_not_copied_but_booking_is_accepted() {
        let use_case = use_case();
        for (student, slot) in [(STUDENT, 17), (OTHER_STUDENT, 18)] {
            let booking = use_case
                .execute(command(student, slot, false))
                .await
                .unwrap();
            assert_eq!(booking.status(), BookingStatus::Pending);
            assert_eq!(booking.lesson_menu(), None, "スロット {slot}");
        }
        assert_eq!(use_case.bookings().all().len(), 2);
        assert_eq!(use_case.analytics().events().len(), 2);
    }

    #[tokio::test]
    async fn double_booking_is_rejected_without_emitting() {
        let use_case = use_case();
        use_case.execute(command(STUDENT, 10, false)).await.unwrap();

        for student in [STUDENT, OTHER_STUDENT] {
            assert_invalid(
                use_case.execute(command(student, 10, false)).await,
                SchedulingError::SlotAlreadyBooked,
            );
        }
        assert_eq!(use_case.bookings().all().len(), 1);
        assert_eq!(use_case.analytics().events().len(), 1);
    }

    #[tokio::test]
    async fn group_slot_takes_bookings_up_to_the_capacity() {
        let use_case = use_case();
        let students = [STUDENT, OTHER_STUDENT, AccountId::from_u128(3)];
        for student in students {
            use_case.execute(command(student, 15, false)).await.unwrap();
        }
        let group = |use_case: &UseCase| {
            use_case
                .bookings()
                .slots()
                .all()
                .into_iter()
                .find(|slot| slot.id() == SlotId::from_u128(15))
                .unwrap()
        };
        assert_eq!(group(&use_case).remaining_seats(), 0);
        assert_eq!(
            group(&use_case).status(),
            SlotStatus::Open,
            "確定するまでは公開中"
        );

        // 4 人目は満席で拒む
        assert_invalid(
            use_case
                .execute(command(AccountId::from_u128(4), 15, false))
                .await,
            SchedulingError::SlotAlreadyBooked,
        );
        assert_eq!(use_case.bookings().all().len(), 3);
        assert_eq!(use_case.analytics().events().len(), 3);
    }

    #[tokio::test]
    async fn the_same_student_cannot_book_a_group_slot_twice() {
        let use_case = use_case();
        use_case.execute(command(STUDENT, 15, false)).await.unwrap();

        assert_invalid(
            use_case.execute(command(STUDENT, 15, false)).await,
            SchedulingError::SlotAlreadyBookedByStudent,
        );
        // 別の生徒は予約できる
        use_case
            .execute(command(OTHER_STUDENT, 15, false))
            .await
            .unwrap();
        assert_eq!(use_case.bookings().all().len(), 2);
        assert_eq!(use_case.analytics().events().len(), 2);
    }

    #[tokio::test]
    async fn trial_booking_is_only_once_per_student() {
        let use_case = use_case();
        use_case.execute(command(STUDENT, 10, true)).await.unwrap();

        assert_invalid(
            use_case.execute(command(STUDENT, 11, true)).await,
            SchedulingError::TrialAlreadyUsed,
        );
        // 通常の予約と、別の生徒の体験予約はできる
        use_case.execute(command(STUDENT, 11, false)).await.unwrap();
        use_case
            .execute(command(OTHER_STUDENT, 14, true))
            .await
            .unwrap();
        assert_eq!(use_case.analytics().events().len(), 3);
    }

    #[tokio::test]
    async fn slot_that_is_not_open_or_missing_is_rejected() {
        let use_case = use_case();
        for (slot, status) in [(12, SlotStatus::Booked), (13, SlotStatus::Closed)] {
            assert_invalid(
                use_case.execute(command(STUDENT, slot, false)).await,
                SchedulingError::SlotNotOpen { status },
            );
        }
        let result = use_case.execute(command(STUDENT, 99, false)).await;
        assert!(
            matches!(result, Err(CreateBookingError::SlotNotFound)),
            "{result:?}"
        );
        assert!(use_case.bookings().all().is_empty());
        assert!(use_case.analytics().events().is_empty());
    }

    /// 保存に必ず失敗する保存先。
    struct BrokenRepository;

    impl BookingRepository for BrokenRepository {
        fn next_id(&self) -> BookingId {
            BookingId::from_u128(1)
        }

        async fn find_slot(&self, _slot_id: SlotId) -> Result<Option<Slot>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }

        async fn find(&self, _booking_id: BookingId) -> Result<Option<Booking>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }

        async fn reserve<F>(
            &self,
            _slot_id: SlotId,
            _student_id: AccountId,
            _reserve: F,
        ) -> Result<Booking, ReserveSlotError>
        where
            F: FnOnce(&mut Slot, &[Booking]) -> Result<Booking, SchedulingError> + Send,
        {
            Err(ReserveSlotError::Unavailable(Unavailable::new(
                "connection refused",
            )))
        }

        async fn update<F>(
            &self,
            _booking_id: BookingId,
            _update: F,
        ) -> Result<Booking, UpdateBookingError>
        where
            F: FnOnce(&mut Booking, &mut Slot) -> Result<(), SchedulingError> + Send,
        {
            Err(UpdateBookingError::Unavailable(Unavailable::new(
                "connection refused",
            )))
        }

        async fn slots_starting_between(
            &self,
            _from: SystemTime,
            _until: SystemTime,
        ) -> Result<(Vec<Slot>, Vec<Booking>), Unavailable> {
            Err(Unavailable::new("connection refused"))
        }

        async fn bookings_of_student(
            &self,
            _student_id: AccountId,
        ) -> Result<Vec<(Booking, Slot)>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }

        async fn last_booked_at_of_students(
            &self,
            _student_ids: &std::collections::BTreeSet<AccountId>,
        ) -> Result<std::collections::BTreeMap<AccountId, SystemTime>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }

        async fn bookings_of_coach(
            &self,
            _coach_id: AccountId,
        ) -> Result<Vec<(Booking, Slot)>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }
    }

    #[tokio::test]
    async fn storage_failure_is_unavailable_and_nothing_is_emitted() {
        let use_case = CreateBooking::new(
            BrokenRepository,
            CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::default()),
            RecordingAnalyticsSink::default(),
            FixedClock(NOW),
        );
        let result = use_case.execute(command(STUDENT, 10, false)).await;
        assert!(
            matches!(result, Err(CreateBookingError::Unavailable(_))),
            "{result:?}"
        );
        assert!(use_case.analytics().events().is_empty());
    }
}
