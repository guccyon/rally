use std::time::SystemTime;

use domain::{
    DomainError,
    identity::{AccountId, Sport},
    scheduling::Slot,
};

use super::{AddToCalendarError, SlotRepository};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
};

/// スロットの公開の入力。`coach_id` は認証したコーチのアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishSlotCommand {
    pub coach_id: AccountId,
    pub sport: Sport,
    pub starts_at: SystemTime,
    pub ends_at: SystemTime,
}

#[derive(Debug)]
pub enum PublishSlotError {
    /// 入力がドメインの不変条件を満たさない（開始が終了より前でない、同じコーチの枠と重なる）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<DomainError> for PublishSlotError {
    fn from(error: DomainError) -> Self {
        Self::Invalid(error)
    }
}

/// スロットの公開のユースケース（コーチだけが呼ぶ。admin-web の `publishSlot`）。
///
/// 枠を作り（開始 < 終了の検証）、コーチのカレンダー（[`domain::scheduling::CoachCalendar`]）で
/// 同じコーチの枠と重ならないことを確かめて保存する。保存したら行動イベント `slot.published`
/// （KPI「コーチの予約枠充足率」の入力）を 1 回発火する。拒否・保存の失敗では発火しない。
pub struct PublishSlot<S, A, C> {
    slots: S,
    analytics: A,
    clock: C,
}

impl<S, A, C> PublishSlot<S, A, C>
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

    pub async fn execute(&self, command: PublishSlotCommand) -> Result<Slot, PublishSlotError> {
        let slot = Slot::publish(
            self.slots.next_id(),
            command.coach_id,
            command.sport,
            command.starts_at,
            command.ends_at,
        )?;

        self.slots
            .add_to_calendar(slot.clone(), |calendar, slot| calendar.publish(slot))
            .await
            .map_err(|error| match error {
                AddToCalendarError::Rejected(error) => PublishSlotError::Invalid(error),
                AddToCalendarError::Unavailable(error) => PublishSlotError::Unavailable(error),
            })?;

        self.analytics
            .emit(AnalyticsEvent::SlotPublished {
                coach_id: slot.coach_id(),
                slot_id: slot.id(),
                starts_at: slot.starts_at(),
                occurred_at: self.clock.now(),
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
    use std::time::Duration;

    use domain::scheduling::{SlotId, SlotStatus};

    use super::*;
    use crate::{
        in_memory::{FixedClock, InMemorySlotRepository, RecordingAnalyticsSink},
        scheduling::SlotCriteria,
    };

    type UseCase = PublishSlot<InMemorySlotRepository, RecordingAnalyticsSink, FixedClock>;

    const NOW: SystemTime = SystemTime::UNIX_EPOCH;
    const COACH: AccountId = AccountId::from_u128(7);

    fn at(hour: u64) -> SystemTime {
        NOW + Duration::from_secs(hour * 3600)
    }

    fn use_case() -> UseCase {
        PublishSlot::new(
            InMemorySlotRepository::default(),
            RecordingAnalyticsSink::default(),
            FixedClock(NOW),
        )
    }

    fn command(coach_id: AccountId, from: u64, to: u64) -> PublishSlotCommand {
        PublishSlotCommand {
            coach_id,
            sport: Sport::Tennis,
            starts_at: at(from),
            ends_at: at(to),
        }
    }

    #[tokio::test]
    async fn publishes_an_open_slot_and_emits_slot_published_once() {
        let use_case = use_case();

        let slot = use_case.execute(command(COACH, 10, 11)).await.unwrap();

        assert_eq!(slot.status(), SlotStatus::Open);
        assert_eq!(slot.coach_id(), COACH);
        assert_eq!(use_case.slots().all(), vec![slot.clone()]);
        let events = use_case.analytics().events();
        assert_eq!(
            events,
            vec![AnalyticsEvent::SlotPublished {
                coach_id: COACH,
                slot_id: slot.id(),
                starts_at: at(10),
                occurred_at: NOW,
            }]
        );
        assert_eq!(events[0].name(), "slot.published");
    }

    #[tokio::test]
    async fn overlapping_slot_of_the_same_coach_is_rejected_without_emitting() {
        let use_case = use_case();
        let first = use_case.execute(command(COACH, 10, 12)).await.unwrap();

        for (from, to) in [(10, 12), (11, 13), (9, 11), (9, 13), (10, 11)] {
            let result = use_case.execute(command(COACH, from, to)).await;
            assert!(
                matches!(
                    &result,
                    Err(PublishSlotError::Invalid(DomainError::SlotOverlaps { existing }))
                        if *existing == first.id()
                ),
                "{from}-{to}: {result:?}"
            );
        }
        assert_eq!(use_case.slots().all().len(), 1);
        assert_eq!(use_case.analytics().events().len(), 1);

        // 隣り合う枠と、別のコーチの同じ時間の枠は公開できる
        use_case.execute(command(COACH, 12, 13)).await.unwrap();
        use_case
            .execute(command(AccountId::from_u128(8), 10, 12))
            .await
            .unwrap();
        assert_eq!(use_case.analytics().events().len(), 3);
    }

    #[tokio::test]
    async fn slot_that_does_not_start_before_it_ends_is_rejected() {
        let use_case = use_case();
        let result = use_case.execute(command(COACH, 10, 10)).await;
        assert!(
            matches!(
                result,
                Err(PublishSlotError::Invalid(
                    DomainError::SlotEndsBeforeItStarts
                ))
            ),
            "{result:?}"
        );
        assert!(use_case.slots().all().is_empty());
        assert!(use_case.analytics().events().is_empty());
    }

    /// 保存に必ず失敗する保存先。
    struct BrokenRepository;

    impl SlotRepository for BrokenRepository {
        fn next_id(&self) -> SlotId {
            SlotId::from_u128(1)
        }

        async fn add_to_calendar<F>(&self, _slot: Slot, _add: F) -> Result<(), AddToCalendarError>
        where
            F: FnOnce(&mut domain::scheduling::CoachCalendar, Slot) -> Result<(), DomainError>
                + Send,
        {
            Err(AddToCalendarError::Unavailable(Unavailable::new(
                "connection refused",
            )))
        }

        async fn search(&self, _criteria: &SlotCriteria) -> Result<Vec<Slot>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }
    }

    #[tokio::test]
    async fn storage_failure_is_unavailable_and_nothing_is_emitted() {
        let use_case = PublishSlot::new(
            BrokenRepository,
            RecordingAnalyticsSink::default(),
            FixedClock(NOW),
        );
        let result = use_case.execute(command(COACH, 10, 11)).await;
        assert!(
            matches!(result, Err(PublishSlotError::Unavailable(_))),
            "{result:?}"
        );
        assert!(use_case.analytics().events().is_empty());
    }
}
