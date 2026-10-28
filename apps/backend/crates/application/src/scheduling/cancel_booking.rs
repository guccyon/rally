use std::time::SystemTime;

use domain::{
    DomainError,
    identity::AccountId,
    scheduling::{Booking, BookingId, CancellationDecision},
};
use uuid::Uuid;

use super::{BookingRepository, UpdateBookingError};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
    domain_event::{DomainEvent, DomainEventPublisher},
};

/// 予約のキャンセルの入力。`student_id` は認証した生徒のアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelBookingCommand {
    pub student_id: AccountId,
    pub booking_id: BookingId,
}

#[derive(Debug)]
pub enum CancelBookingError {
    /// 予約がない
    BookingNotFound,
    /// キャンセルがドメインの不変条件を満たさない（別の生徒の予約、キャンセル済み）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 予約のキャンセルのユースケース（student-app の `cancelBooking`）。
///
/// 予約とそのスロットを読み、domain の [`Booking::cancel`] でキャンセルして（確定済みならスロットを
/// `open` に戻す）、両方を保存する。回数を消費するかはキャンセル規定（domain の `CancellationPolicy`）が
/// 判定し、ここでは判定しない。保存したら判定を載せた境界間イベント `BookingCancelled` を発行し、行動イベント
/// `booking.cancelled` を 1 回発火する。拒否・保存の失敗ではどちらも出さない。受講権の消費は entitlement が
/// `BookingCancelled` を購読して行う。
pub struct CancelBooking<B, A, P, C> {
    bookings: B,
    analytics: A,
    events: P,
    clock: C,
}

impl<B, A, P, C> CancelBooking<B, A, P, C>
where
    B: BookingRepository,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    pub fn new(bookings: B, analytics: A, events: P, clock: C) -> Self {
        Self {
            bookings,
            analytics,
            events,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CancelBookingCommand,
    ) -> Result<Booking, CancelBookingError> {
        let now = self.clock.now();
        let mut decision: Option<CancellationDecision> = None;
        let booking = self
            .bookings
            .update(command.booking_id, |booking, slot| {
                decision = Some(booking.cancel(command.student_id, slot, now)?);
                Ok(())
            })
            .await
            .map_err(|error| match error {
                UpdateBookingError::BookingNotFound => CancelBookingError::BookingNotFound,
                UpdateBookingError::Rejected(error) => CancelBookingError::Invalid(error),
                UpdateBookingError::Unavailable(error) => CancelBookingError::Unavailable(error),
            })?;
        let decision = decision.ok_or_else(|| {
            CancelBookingError::Unavailable(Unavailable::new(
                "保存先がキャンセルを適用せずに成功を返した",
            ))
        })?;

        self.events
            .publish(DomainEvent::BookingCancelled {
                event_id: Uuid::new_v4(),
                occurred_at: now,
                student_id: booking.student_id(),
                coach_id: booking.coach_id(),
                booking_id: booking.id(),
                consumes_quota: decision.consumes_quota,
                cancelled_at: now,
            })
            .await;
        self.analytics
            .emit(AnalyticsEvent::BookingCancelled {
                student_id: booking.student_id(),
                coach_id: booking.coach_id(),
                booking_id: booking.id(),
                slot_id: booking.slot_id(),
                hours_before_start: hours_before_start(booking.starts_at(), now),
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

    pub fn events(&self) -> &P {
        &self.events
    }
}

/// `booking.cancelled` の `hours_before_start`。`at` から `starts_at` までの時間を時間単位で切り捨てる
/// （`at` が開始後なら負）。
fn hours_before_start(starts_at: SystemTime, at: SystemTime) -> i64 {
    const NANOS_PER_HOUR: i128 = 3600 * 1_000_000_000;
    let nanos = match starts_at.duration_since(at) {
        Ok(before) => before.as_nanos() as i128,
        Err(after) => -(after.duration().as_nanos() as i128),
    };
    nanos.div_euclid(NANOS_PER_HOUR) as i64
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::{
        identity::Sport,
        scheduling::{BookingStatus, Slot, SlotId, SlotStatus},
    };

    use super::*;
    use crate::{
        entitlement::EntitlementQuotaGate,
        in_memory::{
            FixedClock, InMemoryBookingRepository, InMemoryEntitlementRepository,
            InMemorySlotRepository, RecordingAnalyticsSink, RecordingDomainEventPublisher,
        },
        scheduling::{ConfirmBooking, ConfirmBookingCommand, CreateBooking, CreateBookingCommand},
    };

    type UseCase = CancelBooking<
        InMemoryBookingRepository,
        RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
        FixedClock,
    >;

    const HOUR: Duration = Duration::from_secs(3600);
    const CREATED_AT: SystemTime = SystemTime::UNIX_EPOCH;
    const COACH: AccountId = AccountId::from_u128(7);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const SLOT: SlotId = SlotId::from_u128(10);

    /// スロットの開始時刻（作成の 100 時間後）
    fn starts_at() -> SystemTime {
        CREATED_AT + 100 * HOUR
    }

    /// 公開中のスロットを 1 つ置き、生徒がそれを予約した状態（`confirm` なら確定済み）から始め、
    /// 開始の `before_start` 前にキャンセルするユースケースを返す。
    async fn use_case(confirm: bool, before_start: Duration) -> (UseCase, Booking) {
        let slots = InMemorySlotRepository::with_slots([Slot::publish(
            SLOT,
            COACH,
            Sport::Tennis,
            starts_at(),
            starts_at() + HOUR,
            domain::scheduling::SlotCapacity::ONE,
        )
        .unwrap()]);
        let bookings = InMemoryBookingRepository::new(slots);
        let mut booking = CreateBooking::new(
            bookings.clone(),
            RecordingAnalyticsSink::default(),
            FixedClock(CREATED_AT),
        )
        .execute(CreateBookingCommand {
            student_id: STUDENT,
            slot_id: SLOT,
            is_trial: false,
        })
        .await
        .unwrap();
        if confirm {
            booking = ConfirmBooking::new(
                bookings.clone(),
                EntitlementQuotaGate::new(InMemoryEntitlementRepository::default()),
                RecordingAnalyticsSink::default(),
                RecordingDomainEventPublisher::default(),
                FixedClock(CREATED_AT),
            )
            .execute(ConfirmBookingCommand {
                student_id: STUDENT,
                booking_id: booking.id(),
            })
            .await
            .unwrap();
        }
        let use_case = CancelBooking::new(
            bookings,
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            FixedClock(starts_at() - before_start),
        );
        (use_case, booking)
    }

    fn command(student_id: AccountId, booking_id: BookingId) -> CancelBookingCommand {
        CancelBookingCommand {
            student_id,
            booking_id,
        }
    }

    fn consumes_quota(use_case: &UseCase) -> bool {
        match use_case.events().events().as_slice() {
            [DomainEvent::BookingCancelled { consumes_quota, .. }] => *consumes_quota,
            other => panic!("BookingCancelled が 1 件でない: {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancelling_reopens_the_slot_and_publishes_booking_cancelled() {
        let before_start = 23 * HOUR;
        let (use_case, confirmed) = use_case(true, before_start).await;
        let cancelled_at = starts_at() - before_start;

        let booking = use_case
            .execute(command(STUDENT, confirmed.id()))
            .await
            .unwrap();

        assert_eq!(booking.status(), BookingStatus::Cancelled);
        assert_eq!(booking.cancelled_at(), Some(cancelled_at));
        assert_eq!(use_case.bookings().all(), vec![booking.clone()]);
        assert_eq!(
            use_case.bookings().slots().all()[0].status(),
            SlotStatus::Open,
            "キャンセルでスロットが公開中に戻る"
        );

        let events = use_case.events().events();
        assert_eq!(events.len(), 1, "BookingCancelled は 1 回だけ発行する");
        let DomainEvent::BookingCancelled {
            event_id,
            occurred_at,
            student_id,
            coach_id,
            booking_id,
            consumes_quota,
            cancelled_at: event_cancelled_at,
        } = &events[0]
        else {
            panic!("{:?}", events[0]);
        };
        assert!(!event_id.is_nil());
        assert_eq!(
            (
                *occurred_at,
                *student_id,
                *coach_id,
                *booking_id,
                *event_cancelled_at
            ),
            (cancelled_at, STUDENT, COACH, booking.id(), cancelled_at)
        );
        assert!(
            *consumes_quota,
            "開始の 23 時間前のキャンセルは回数を消費する"
        );
        assert_eq!(events[0].name(), "BookingCancelled");

        let analytics = use_case.analytics().events();
        assert_eq!(
            analytics,
            vec![AnalyticsEvent::BookingCancelled {
                student_id: STUDENT,
                coach_id: COACH,
                booking_id: booking.id(),
                slot_id: SLOT,
                hours_before_start: 23,
                occurred_at: cancelled_at,
            }]
        );
        assert_eq!(analytics[0].name(), "booking.cancelled");
    }

    #[tokio::test]
    async fn cancelling_one_of_a_full_group_slot_reopens_it() {
        let slots = InMemorySlotRepository::with_slots([Slot::publish(
            SLOT,
            COACH,
            Sport::Tennis,
            starts_at(),
            starts_at() + HOUR,
            domain::scheduling::SlotCapacity::new(2).unwrap(),
        )
        .unwrap()]);
        let bookings = InMemoryBookingRepository::new(slots);
        let create = CreateBooking::new(
            bookings.clone(),
            RecordingAnalyticsSink::default(),
            FixedClock(CREATED_AT),
        );
        let confirm = ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(InMemoryEntitlementRepository::default()),
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            FixedClock(CREATED_AT),
        );
        let slot = || bookings.slots().all()[0].clone();
        let mut confirmed = Vec::new();
        for student_id in [STUDENT, OTHER_STUDENT] {
            let booking = create
                .execute(CreateBookingCommand {
                    student_id,
                    slot_id: SLOT,
                    is_trial: false,
                })
                .await
                .unwrap();
            assert_eq!(slot().status(), SlotStatus::Open);
            confirmed.push(
                confirm
                    .execute(ConfirmBookingCommand {
                        student_id,
                        booking_id: booking.id(),
                    })
                    .await
                    .unwrap(),
            );
        }
        assert_eq!(
            slot().status(),
            SlotStatus::Booked,
            "確定した予約が定員に達した"
        );

        let use_case = CancelBooking::new(
            bookings.clone(),
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            FixedClock(starts_at() - 48 * HOUR),
        );
        use_case
            .execute(command(OTHER_STUDENT, confirmed[1].id()))
            .await
            .unwrap();
        assert_eq!(
            slot().status(),
            SlotStatus::Open,
            "空きができたら公開中に戻る"
        );
        assert_eq!(slot().remaining_seats(), 1);
        assert_eq!(
            bookings
                .find(confirmed[0].id())
                .await
                .unwrap()
                .unwrap()
                .status(),
            BookingStatus::Confirmed,
            "ほかの生徒の予約はそのまま"
        );
    }

    #[tokio::test]
    async fn consumes_quota_follows_the_cancellation_policy() {
        for (confirm, before_start, expected) in [
            (true, 23 * HOUR, true),
            (true, 25 * HOUR, false),
            (false, 23 * HOUR, true),
            (false, 25 * HOUR, false),
        ] {
            let (use_case, booking) = use_case(confirm, before_start).await;
            use_case
                .execute(command(STUDENT, booking.id()))
                .await
                .unwrap();
            assert_eq!(
                consumes_quota(&use_case),
                expected,
                "確定済み {confirm}・開始の {before_start:?} 前"
            );
            assert_eq!(
                use_case.bookings().slots().all()[0].status(),
                SlotStatus::Open
            );
        }
    }

    #[tokio::test]
    async fn rejected_cancellation_changes_nothing_and_emits_nothing() {
        let (use_case, confirmed) = use_case(true, 25 * HOUR).await;

        let result = use_case
            .execute(command(OTHER_STUDENT, confirmed.id()))
            .await;
        assert!(
            matches!(
                result,
                Err(CancelBookingError::Invalid(
                    DomainError::BookingOfAnotherStudent
                ))
            ),
            "{result:?}"
        );
        let result = use_case
            .execute(command(STUDENT, BookingId::from_u128(99)))
            .await;
        assert!(
            matches!(result, Err(CancelBookingError::BookingNotFound)),
            "{result:?}"
        );
        assert_eq!(use_case.bookings().all(), vec![confirmed.clone()]);
        assert_eq!(
            use_case.bookings().slots().all()[0].status(),
            SlotStatus::Booked
        );
        assert!(use_case.events().events().is_empty());
        assert!(use_case.analytics().events().is_empty());

        // 2 回目のキャンセルは拒否し、イベントは最初の 1 回だけ
        use_case
            .execute(command(STUDENT, confirmed.id()))
            .await
            .unwrap();
        let result = use_case.execute(command(STUDENT, confirmed.id())).await;
        assert!(
            matches!(
                result,
                Err(CancelBookingError::Invalid(
                    DomainError::BookingAlreadyCancelled
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.events().events().len(), 1);
        assert_eq!(use_case.analytics().events().len(), 1);
    }

    #[test]
    fn hours_before_start_rounds_down() {
        let start = starts_at();
        let minute = Duration::from_secs(60);
        assert_eq!(hours_before_start(start, start - 23 * HOUR), 23);
        assert_eq!(
            hours_before_start(start, start - 23 * HOUR - 59 * minute),
            23
        );
        assert_eq!(hours_before_start(start, start - minute), 0);
        assert_eq!(hours_before_start(start, start), 0);
        assert_eq!(hours_before_start(start, start + minute), -1);
        assert_eq!(hours_before_start(start, start + HOUR), -1);
        assert_eq!(hours_before_start(start, start + HOUR + minute), -2);
    }
}
