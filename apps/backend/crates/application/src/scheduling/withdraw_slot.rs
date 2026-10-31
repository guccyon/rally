use domain::{
    identity::AccountId,
    scheduling::{SchedulingError, Slot, SlotId},
};

use super::{SlotRepository, UpdateSlotError};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
};

/// 枠の取り下げの入力。`coach_id` は認証したコーチのアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawSlotCommand {
    pub coach_id: AccountId,
    pub slot_id: SlotId,
}

#[derive(Debug)]
pub enum WithdrawSlotError {
    /// スロットがない
    SlotNotFound,
    /// 取り下げがドメインの不変条件を満たさない（別のコーチの枠、すでに閉じた枠、予約が入っている枠、始まった枠）
    Invalid(SchedulingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// コーチが公開した枠の取り下げのユースケース（コーチだけが呼ぶ。admin-web の `withdrawMySlot`）。
///
/// 枠を読み、domain の [`Slot::withdraw`] で取り下げられるか（自分の・公開中の・有効な予約がない・まだ始まっていない枠か）を
/// 確かめて閉じ（`closed`）、保存する。ここでは判定しない。保存したら行動イベント `slot.withdrawn`（KPI「コーチの予約枠
/// 充足率」の入力。取り下げた枠を分母から除く）を 1 回発火する。拒否・保存の失敗では発火しない。予約の入っていない枠
/// だけを閉じるので、境界間イベントは発行しない（他のコンテキストに知らせることがない）。
pub struct WithdrawSlot<S, A, C> {
    slots: S,
    analytics: A,
    clock: C,
}

impl<S, A, C> WithdrawSlot<S, A, C>
where
    S: SlotRepository,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(slots: S, analytics: A, clock: C) -> Self {
        Self {
            slots,
            analytics,
            clock,
        }
    }

    pub async fn execute(&self, command: WithdrawSlotCommand) -> Result<Slot, WithdrawSlotError> {
        let now = self.clock.now();
        let slot = self
            .slots
            .update(command.slot_id, |slot| slot.withdraw(command.coach_id, now))
            .await
            .map_err(|error| match error {
                UpdateSlotError::SlotNotFound => WithdrawSlotError::SlotNotFound,
                UpdateSlotError::Rejected(error) => WithdrawSlotError::Invalid(error),
                UpdateSlotError::Unavailable(error) => WithdrawSlotError::Unavailable(error),
            })?;

        self.analytics
            .emit(AnalyticsEvent::SlotWithdrawn {
                coach_id: slot.coach_id(),
                slot_id: slot.id(),
                starts_at: slot.starts_at(),
                occurred_at: now,
            })
            .await;

        Ok(slot)
    }

    pub fn slots(&self) -> &S {
        &self.slots
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
        scheduling::{SlotCapacity, SlotStatus},
    };

    use super::*;
    use crate::{
        catalog::CatalogLessonMenuLookup,
        in_memory::{
            FixedClock, InMemoryBookingRepository, InMemoryLessonMenuRepository,
            InMemorySlotRepository, RecordingAnalyticsSink, RecordingDomainEventPublisher,
        },
        scheduling::{
            CancelBooking, CancelBookingCommand, CreateBooking, CreateBookingCommand, PublishSlot,
            PublishSlotCommand,
        },
    };

    type UseCase = WithdrawSlot<InMemorySlotRepository, RecordingAnalyticsSink, FixedClock>;

    const HOUR: Duration = Duration::from_secs(3600);
    const NOW: SystemTime = SystemTime::UNIX_EPOCH;
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const SLOT: SlotId = SlotId::from_u128(10);

    fn at(hour: u32) -> SystemTime {
        NOW + hour * HOUR
    }

    /// `COACH` の公開中の枠 `SLOT`（開始 `from` 時、1 時間、定員 1）を 1 つ置いた保存先。
    fn slots_with_open_slot(from: u32) -> InMemorySlotRepository {
        InMemorySlotRepository::with_slots([Slot::publish(
            SLOT,
            COACH,
            Sport::Tennis,
            at(from),
            at(from + 1),
            SlotCapacity::ONE,
        )
        .unwrap()])
    }

    fn use_case(slots: InMemorySlotRepository, now: SystemTime) -> UseCase {
        WithdrawSlot::new(slots, RecordingAnalyticsSink::default(), FixedClock(now))
    }

    fn command(coach_id: AccountId, slot_id: SlotId) -> WithdrawSlotCommand {
        WithdrawSlotCommand { coach_id, slot_id }
    }

    fn create_booking(
        bookings: &InMemoryBookingRepository,
    ) -> CreateBooking<
        InMemoryBookingRepository,
        CatalogLessonMenuLookup<InMemoryLessonMenuRepository>,
        RecordingAnalyticsSink,
        FixedClock,
    > {
        CreateBooking::new(
            bookings.clone(),
            CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::default()),
            RecordingAnalyticsSink::default(),
            FixedClock(NOW),
        )
    }

    #[tokio::test]
    async fn closes_the_slot_and_emits_slot_withdrawn_once() {
        let use_case = use_case(slots_with_open_slot(10), at(1));

        let slot = use_case.execute(command(COACH, SLOT)).await.unwrap();

        assert_eq!(slot.id(), SLOT);
        assert_eq!(slot.status(), SlotStatus::Closed);
        assert_eq!(use_case.slots().all(), vec![slot]);
        let events = use_case.analytics().events();
        assert_eq!(
            events,
            vec![AnalyticsEvent::SlotWithdrawn {
                coach_id: COACH,
                slot_id: SLOT,
                starts_at: at(10),
                occurred_at: at(1),
            }]
        );
        assert_eq!(events[0].name(), "slot.withdrawn");
    }

    #[tokio::test]
    async fn the_withdrawn_slot_no_longer_occupies_the_time() {
        let slots = slots_with_open_slot(10);
        use_case(slots.clone(), at(1))
            .execute(command(COACH, SLOT))
            .await
            .unwrap();

        let republished = PublishSlot::new(
            slots.clone(),
            CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::default()),
            RecordingAnalyticsSink::default(),
            FixedClock(at(1)),
        )
        .execute(PublishSlotCommand {
            coach_id: COACH,
            sport: Sport::Tennis,
            starts_at: at(10),
            ends_at: at(11),
            capacity: SlotCapacity::ONE,
            lesson_menu_id: None,
        })
        .await
        .unwrap();
        assert_eq!(republished.status(), SlotStatus::Open);
    }

    #[tokio::test]
    async fn a_slot_whose_booking_was_cancelled_can_be_withdrawn() {
        let slots = slots_with_open_slot(100);
        let bookings = InMemoryBookingRepository::new(slots.clone());
        let booking = create_booking(&bookings)
            .execute(CreateBookingCommand {
                student_id: STUDENT,
                slot_id: SLOT,
                is_trial: false,
            })
            .await
            .unwrap();
        CancelBooking::new(
            bookings.clone(),
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            FixedClock(NOW),
        )
        .execute(CancelBookingCommand {
            student_id: STUDENT,
            booking_id: booking.id(),
        })
        .await
        .unwrap();

        let slot = use_case(slots, NOW)
            .execute(command(COACH, SLOT))
            .await
            .unwrap();
        assert_eq!(slot.status(), SlotStatus::Closed);
    }

    #[tokio::test]
    async fn rejects_without_changing_anything_or_emitting() {
        // 予約の入った枠
        let booked_slots = slots_with_open_slot(100);
        let bookings = InMemoryBookingRepository::new(booked_slots.clone());
        create_booking(&bookings)
            .execute(CreateBookingCommand {
                student_id: STUDENT,
                slot_id: SLOT,
                is_trial: false,
            })
            .await
            .unwrap();
        // すでに閉じた枠
        let closed_slots = slots_with_open_slot(10);
        use_case(closed_slots.clone(), NOW)
            .execute(command(COACH, SLOT))
            .await
            .unwrap();

        for (slots, now, coach, expected) in [
            (
                slots_with_open_slot(10),
                NOW,
                OTHER_COACH,
                SchedulingError::SlotOfAnotherCoach,
            ),
            (
                booked_slots,
                NOW,
                COACH,
                SchedulingError::SlotHasActiveBookings { active: 1 },
            ),
            (
                slots_with_open_slot(10),
                at(10),
                COACH,
                SchedulingError::SlotAlreadyStarted,
            ),
            (
                closed_slots,
                NOW,
                COACH,
                SchedulingError::SlotNotOpen {
                    status: SlotStatus::Closed,
                },
            ),
        ] {
            let before = slots.all();
            let use_case = use_case(slots, now);
            let error = use_case.execute(command(coach, SLOT)).await.unwrap_err();
            assert!(
                matches!(&error, WithdrawSlotError::Invalid(e) if *e == expected),
                "{error:?}"
            );
            assert_eq!(use_case.slots().all(), before);
            assert_eq!(use_case.analytics().events(), vec![]);
        }
    }

    #[tokio::test]
    async fn unknown_slot_is_not_found() {
        let use_case = use_case(slots_with_open_slot(10), NOW);

        let error = use_case
            .execute(command(COACH, SlotId::from_u128(99)))
            .await
            .unwrap_err();

        assert!(
            matches!(error, WithdrawSlotError::SlotNotFound),
            "{error:?}"
        );
        assert_eq!(use_case.analytics().events(), vec![]);
    }
}
