use domain::{
    DomainError,
    identity::AccountId,
    scheduling::{Booking, SlotId},
};

use super::{BookingRepository, ReserveSlotError};
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
    /// 予約がドメインの不変条件を満たさない（公開中でない、二重予約、2 回目の体験予約）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 予約の作成のユースケース（体験予約を含む。student-app の `createBooking`）。
///
/// スロットとその予約・生徒の予約を読み、domain の [`Booking::reserve`] で不変条件（公開中のスロット
/// だけ、1 スロット 1 予約、体験予約は初回だけ）を確かめて、確定前（`pending`）の予約を保存する。
/// スロットはまだ埋めない（確定で埋まる。[`super::ConfirmBooking`]）。受講権も消費しない。
/// 保存したら行動イベント `booking.created` を 1 回発火する。拒否・保存の失敗では発火しない。
pub struct CreateBooking<B, A, C> {
    bookings: B,
    analytics: A,
    clock: C,
}

impl<B, A, C> CreateBooking<B, A, C>
where
    B: BookingRepository,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(bookings: B, analytics: A, clock: C) -> Self {
        Self {
            bookings,
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

        let booking = self
            .bookings
            .reserve(slot_id, student_id, |slot, existing| {
                Booking::reserve(id, student_id, is_trial, slot, existing, now)
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
        identity::Sport,
        scheduling::{BookingId, BookingStatus, Slot, SlotStatus},
    };

    use super::*;
    use crate::{
        in_memory::{
            FixedClock, InMemoryBookingRepository, InMemorySlotRepository, RecordingAnalyticsSink,
        },
        scheduling::UpdateBookingError,
    };

    type UseCase = CreateBooking<InMemoryBookingRepository, RecordingAnalyticsSink, FixedClock>;

    const NOW: SystemTime = SystemTime::UNIX_EPOCH;
    const COACH: AccountId = AccountId::from_u128(7);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);

    fn slot(id: u128, status: SlotStatus) -> Slot {
        let starts_at = NOW + Duration::from_secs(id as u64 * 3600);
        Slot::restore(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            starts_at,
            starts_at + Duration::from_secs(3600),
            status,
        )
        .unwrap()
    }

    /// スロット 10・11・14 は公開中、12 は予約で埋まり、13 は閉じている。
    fn use_case() -> UseCase {
        let slots = InMemorySlotRepository::with_slots([
            slot(10, SlotStatus::Open),
            slot(11, SlotStatus::Open),
            slot(12, SlotStatus::Booked),
            slot(13, SlotStatus::Closed),
            slot(14, SlotStatus::Open),
        ]);
        CreateBooking::new(
            InMemoryBookingRepository::new(slots),
            RecordingAnalyticsSink::default(),
            FixedClock(NOW),
        )
    }

    fn command(student_id: AccountId, slot: u128, is_trial: bool) -> CreateBookingCommand {
        CreateBookingCommand {
            student_id,
            slot_id: SlotId::from_u128(slot),
            is_trial,
        }
    }

    fn assert_invalid(result: Result<Booking, CreateBookingError>, expected: DomainError) {
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
    async fn double_booking_is_rejected_without_emitting() {
        let use_case = use_case();
        use_case.execute(command(STUDENT, 10, false)).await.unwrap();

        for student in [STUDENT, OTHER_STUDENT] {
            assert_invalid(
                use_case.execute(command(student, 10, false)).await,
                DomainError::SlotAlreadyBooked,
            );
        }
        assert_eq!(use_case.bookings().all().len(), 1);
        assert_eq!(use_case.analytics().events().len(), 1);
    }

    #[tokio::test]
    async fn trial_booking_is_only_once_per_student() {
        let use_case = use_case();
        use_case.execute(command(STUDENT, 10, true)).await.unwrap();

        assert_invalid(
            use_case.execute(command(STUDENT, 11, true)).await,
            DomainError::TrialAlreadyUsed,
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
                DomainError::SlotNotOpen { status },
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
            F: FnOnce(&Slot, &[Booking]) -> Result<Booking, DomainError> + Send,
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
            F: FnOnce(&mut Booking, &mut Slot) -> Result<(), DomainError> + Send,
        {
            Err(UpdateBookingError::Unavailable(Unavailable::new(
                "connection refused",
            )))
        }
    }

    #[tokio::test]
    async fn storage_failure_is_unavailable_and_nothing_is_emitted() {
        let use_case = CreateBooking::new(
            BrokenRepository,
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
