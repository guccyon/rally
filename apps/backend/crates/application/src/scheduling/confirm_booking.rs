use domain::{
    DomainError,
    identity::AccountId,
    scheduling::{Booking, BookingId},
};
use uuid::Uuid;

use super::{BookingRepository, QuotaGate, UpdateBookingError};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
    domain_event::{DomainEvent, DomainEventPublisher},
};

/// 予約の確定の入力。`student_id` は認証した生徒のアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmBookingCommand {
    pub student_id: AccountId,
    pub booking_id: BookingId,
}

#[derive(Debug)]
pub enum ConfirmBookingError {
    /// 予約がない
    BookingNotFound,
    /// 確定がドメインの不変条件を満たさない（別の生徒の予約、確定前でない、スロットが公開中でない、
    /// ビデオ通話の暦月の枠が残っていない（[`DomainError::LiveLessonQuotaExceeded`]））
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 予約の確定のユースケース（student-app の `confirmBooking`）。
///
/// 予約とそのスロットを読み、domain の [`Booking::confirm`] で確定してスロットを `booked` にし、
/// 両方を保存する。確定の前に entitlement に「ビデオ通話をもう 1 回予約してよいか」を問い合わせ（[`QuotaGate`]）、
/// 否なら確定を拒否する（不変条件 1。判定の規則は entitlement 側）。保存したら境界間イベント
/// `BookingConfirmed` を発行し（entitlement はこれを購読して枠を使う）、行動イベント `booking.confirmed` を
/// 1 回発火する。拒否・保存の失敗ではどちらも出さない。
///
/// 問い合わせと枠の消費は別の手順なので、同じ生徒の確定が並行すると両方が問い合わせを通りうる（そのとき
/// 後の消費は entitlement で拒否され、購読側の失敗として記録される）。Gen1 の生徒アプリからの操作では
/// 起きにくいので許す。
pub struct ConfirmBooking<B, G, A, P, C> {
    bookings: B,
    quota: G,
    analytics: A,
    events: P,
    clock: C,
}

impl<B, G, A, P, C> ConfirmBooking<B, G, A, P, C>
where
    B: BookingRepository,
    G: QuotaGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    pub fn new(bookings: B, quota: G, analytics: A, events: P, clock: C) -> Self {
        Self {
            bookings,
            quota,
            analytics,
            events,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: ConfirmBookingCommand,
    ) -> Result<Booking, ConfirmBookingError> {
        let now = self.clock.now();
        let may_book = self
            .quota
            .may_book_live_lesson(command.student_id, now)
            .await
            .map_err(ConfirmBookingError::Unavailable)?;
        let booking = self
            .bookings
            .update(command.booking_id, |booking, slot| {
                // 別の生徒の予約・確定前でない予約の拒否を先に返す（枠の超過より優先する）
                booking.confirm(command.student_id, slot, now)?;
                if may_book {
                    Ok(())
                } else {
                    Err(DomainError::LiveLessonQuotaExceeded)
                }
            })
            .await
            .map_err(|error| match error {
                UpdateBookingError::BookingNotFound => ConfirmBookingError::BookingNotFound,
                UpdateBookingError::Rejected(error) => ConfirmBookingError::Invalid(error),
                UpdateBookingError::Unavailable(error) => ConfirmBookingError::Unavailable(error),
            })?;

        self.events
            .publish(DomainEvent::BookingConfirmed {
                event_id: Uuid::new_v4(),
                occurred_at: now,
                student_id: booking.student_id(),
                coach_id: booking.coach_id(),
                booking_id: booking.id(),
            })
            .await;
        self.analytics
            .emit(AnalyticsEvent::BookingConfirmed {
                student_id: booking.student_id(),
                coach_id: booking.coach_id(),
                booking_id: booking.id(),
                slot_id: booking.slot_id(),
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        entitlement::{
            Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan,
        },
        identity::Sport,
        scheduling::{BookingStatus, Slot, SlotId, SlotStatus},
    };

    use super::*;
    use crate::{
        entitlement::{EntitlementQuotaGate, calendar_month},
        in_memory::{
            FixedClock, InMemoryBookingRepository, InMemoryEntitlementRepository,
            InMemorySlotRepository, RecordingAnalyticsSink, RecordingDomainEventPublisher,
        },
        scheduling::{CreateBooking, CreateBookingCommand},
    };

    type UseCase = ConfirmBooking<
        InMemoryBookingRepository,
        EntitlementQuotaGate<InMemoryEntitlementRepository>,
        RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
        FixedClock,
    >;

    const CREATED_AT: SystemTime = SystemTime::UNIX_EPOCH;
    /// 確定する時刻（作成の 1 分後）
    fn now() -> SystemTime {
        CREATED_AT + Duration::from_secs(60)
    }
    const COACH: AccountId = AccountId::from_u128(7);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const SLOT: SlotId = SlotId::from_u128(10);

    /// 公開中のスロットを 1 つ置き、生徒がそれを体験予約した状態から始める。
    async fn use_case() -> (UseCase, Booking) {
        use_case_with(InMemoryEntitlementRepository::default()).await
    }

    /// [`use_case`] の受講権の保存先を `entitlements` にする。
    async fn use_case_with(entitlements: InMemoryEntitlementRepository) -> (UseCase, Booking) {
        let starts_at = CREATED_AT + Duration::from_secs(36_000);
        let slots = InMemorySlotRepository::with_slots([Slot::publish(
            SLOT,
            COACH,
            Sport::Tennis,
            starts_at,
            starts_at + Duration::from_secs(3600),
        )
        .unwrap()]);
        let bookings = InMemoryBookingRepository::new(slots);
        let booking = CreateBooking::new(
            bookings.clone(),
            RecordingAnalyticsSink::default(),
            FixedClock(CREATED_AT),
        )
        .execute(CreateBookingCommand {
            student_id: STUDENT,
            slot_id: SLOT,
            is_trial: true,
        })
        .await
        .unwrap();
        let use_case = ConfirmBooking::new(
            bookings,
            EntitlementQuotaGate::new(entitlements),
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            FixedClock(now()),
        );
        (use_case, booking)
    }

    fn command(student_id: AccountId, booking_id: BookingId) -> ConfirmBookingCommand {
        ConfirmBookingCommand {
            student_id,
            booking_id,
        }
    }

    #[tokio::test]
    async fn confirming_books_the_slot_and_publishes_booking_confirmed() {
        let (use_case, pending) = use_case().await;

        let booking = use_case
            .execute(command(STUDENT, pending.id()))
            .await
            .unwrap();

        assert_eq!(booking.status(), BookingStatus::Confirmed);
        assert_eq!(booking.confirmed_at(), Some(now()));
        assert_eq!(use_case.bookings().all(), vec![booking.clone()]);
        assert_eq!(
            use_case.bookings().slots().all()[0].status(),
            SlotStatus::Booked,
            "確定でスロットが埋まる"
        );

        let events = use_case.events().events();
        assert_eq!(events.len(), 1, "BookingConfirmed は 1 回だけ発行する");
        let DomainEvent::BookingConfirmed {
            event_id,
            occurred_at,
            student_id,
            coach_id,
            booking_id,
        } = &events[0]
        else {
            panic!("{:?}", events[0]);
        };
        assert!(!event_id.is_nil());
        assert_eq!(
            (*occurred_at, *student_id, *coach_id, *booking_id),
            (now(), STUDENT, COACH, booking.id())
        );
        assert_eq!(events[0].name(), "BookingConfirmed");

        let analytics = use_case.analytics().events();
        assert_eq!(
            analytics,
            vec![AnalyticsEvent::BookingConfirmed {
                student_id: STUDENT,
                coach_id: COACH,
                booking_id: booking.id(),
                slot_id: SLOT,
                occurred_at: now(),
            }]
        );
        assert_eq!(analytics[0].name(), "booking.confirmed");
    }

    #[tokio::test]
    async fn rejected_confirmation_changes_nothing_and_emits_nothing() {
        let (use_case, pending) = use_case().await;

        let result = use_case.execute(command(OTHER_STUDENT, pending.id())).await;
        assert!(
            matches!(
                result,
                Err(ConfirmBookingError::Invalid(
                    DomainError::BookingOfAnotherStudent
                ))
            ),
            "{result:?}"
        );
        let result = use_case
            .execute(command(STUDENT, BookingId::from_u128(99)))
            .await;
        assert!(
            matches!(result, Err(ConfirmBookingError::BookingNotFound)),
            "{result:?}"
        );
        assert_eq!(use_case.bookings().all(), vec![pending.clone()]);
        assert_eq!(
            use_case.bookings().slots().all()[0].status(),
            SlotStatus::Open
        );
        assert!(use_case.events().events().is_empty());
        assert!(use_case.analytics().events().is_empty());

        // 2 回目の確定は拒否し、イベントは最初の 1 回だけ
        use_case
            .execute(command(STUDENT, pending.id()))
            .await
            .unwrap();
        let result = use_case.execute(command(STUDENT, pending.id())).await;
        assert!(
            matches!(
                result,
                Err(ConfirmBookingError::Invalid(
                    DomainError::BookingNotPending {
                        status: BookingStatus::Confirmed
                    }
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.events().events().len(), 1);
        assert_eq!(use_case.analytics().events().len(), 1);
    }

    #[tokio::test]
    async fn confirmation_without_live_lesson_quota_left_is_rejected_and_changes_nothing() {
        // 確定の時刻（1970-01）の暦月の枠を予約 1・2 で使い切った月額の生徒
        let used_up = Entitlement::monthly(
            EntitlementId::from_u128(30),
            STUDENT,
            Sport::Tennis,
            SubscriptionId::from_u128(40),
            LiveLessonQuota::restore(
                SubscriptionPlan::Standard,
                calendar_month::of(now()),
                [BookingId::from_u128(1), BookingId::from_u128(2)],
            )
            .unwrap(),
        )
        .unwrap();
        let (use_case, pending) =
            use_case_with(InMemoryEntitlementRepository::with_entitlements([used_up])).await;

        let result = use_case.execute(command(STUDENT, pending.id())).await;
        assert!(
            matches!(
                result,
                Err(ConfirmBookingError::Invalid(
                    DomainError::LiveLessonQuotaExceeded
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.bookings().all(), vec![pending.clone()]);
        assert_eq!(
            use_case.bookings().slots().all()[0].status(),
            SlotStatus::Open
        );
        assert!(use_case.events().events().is_empty());
        assert!(use_case.analytics().events().is_empty());

        // 別の生徒の予約は、枠の超過より先に別の生徒の予約として拒否する
        let result = use_case.execute(command(OTHER_STUDENT, pending.id())).await;
        assert!(
            matches!(
                result,
                Err(ConfirmBookingError::Invalid(
                    DomainError::BookingOfAnotherStudent
                ))
            ),
            "{result:?}"
        );
    }
}
