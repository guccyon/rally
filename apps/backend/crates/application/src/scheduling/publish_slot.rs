use std::{collections::BTreeSet, time::SystemTime};

use domain::{
    catalog::LessonMenuId,
    catalog::Sport,
    identity::AccountId,
    scheduling::{SchedulingError, Slot, SlotCapacity},
};

use super::{AddToCalendarError, LessonMenuLookup, SlotRepository};
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
    /// 定員（1〜6。契約で省かれたら境界が [`SlotCapacity::default`] の 1 を渡す）
    pub capacity: SlotCapacity,
    /// 枠に結びつけるレッスンメニュー（任意。なければメニューのない枠）
    pub lesson_menu_id: Option<LessonMenuId>,
}

#[derive(Debug)]
pub enum PublishSlotError {
    /// 入力がドメインの不変条件を満たさない（開始が終了より前でない、同じコーチの枠と重なる、結びつけるメニューが
    /// 枠に合わない）
    Invalid(SchedulingError),
    /// 結びつけるレッスンメニューがない
    LessonMenuNotFound,
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<SchedulingError> for PublishSlotError {
    fn from(error: SchedulingError) -> Self {
        Self::Invalid(error)
    }
}

/// スロットの公開のユースケース（コーチだけが呼ぶ。admin-web の `publishSlot`）。
///
/// 定員つきの枠を作り（開始 < 終了の検証）、コーチのカレンダー（[`domain::scheduling::CoachCalendar`]）で
/// 同じコーチの枠と重ならないことを確かめて保存する。レッスンメニューの指定があれば、catalog に問い合わせて
/// （[`LessonMenuLookup`]）枠に結びつけられるか（[`Slot::with_lesson_menu`]）を確かめる。保存したら行動イベント
/// `slot.published`（KPI「コーチの予約枠充足率」の入力）を 1 回発火する。拒否・保存の失敗では発火しない。
pub struct PublishSlot<S, M, A, C> {
    slots: S,
    lesson_menus: M,
    analytics: A,
    clock: C,
}

impl<S, M, A, C> PublishSlot<S, M, A, C>
where
    S: SlotRepository,
    M: LessonMenuLookup,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(slots: S, lesson_menus: M, analytics: A, clock: C) -> Self {
        Self {
            slots,
            lesson_menus,
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
            command.capacity,
        )?;
        let slot = match command.lesson_menu_id {
            None => slot,
            Some(id) => {
                let menus = self
                    .lesson_menus
                    .find_lesson_menus(&BTreeSet::from([id]))
                    .await
                    .map_err(PublishSlotError::Unavailable)?;
                let menu = menus.get(&id).ok_or(PublishSlotError::LessonMenuNotFound)?;
                slot.with_lesson_menu(menu)?
            }
        };

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

    use domain::{
        catalog::{
            LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
            LessonMenuName,
        },
        scheduling::{SlotId, SlotStatus},
    };

    use super::*;
    use crate::{
        catalog::CatalogLessonMenuLookup,
        in_memory::{
            FixedClock, InMemoryLessonMenuRepository, InMemorySlotRepository,
            RecordingAnalyticsSink,
        },
        scheduling::SlotCriteria,
    };

    type Menus = CatalogLessonMenuLookup<InMemoryLessonMenuRepository>;
    type UseCase = PublishSlot<InMemorySlotRepository, Menus, RecordingAnalyticsSink, FixedClock>;

    const NOW: SystemTime = SystemTime::UNIX_EPOCH;
    const COACH: AccountId = AccountId::from_u128(7);

    fn at(hour: u64) -> SystemTime {
        NOW + Duration::from_secs(hour * 3600)
    }

    /// `COACH` のテニスの公開中の 60 分のメニュー
    const MENU: LessonMenuId = LessonMenuId::from_u128(50);
    /// `COACH` のテニスの非公開の 60 分のメニュー
    const UNPUBLISHED_MENU: LessonMenuId = LessonMenuId::from_u128(51);
    /// `COACH` のテニスの非同期のメニュー
    const ASYNC_MENU: LessonMenuId = LessonMenuId::from_u128(52);
    /// 別のコーチのテニスの公開中の 60 分のメニュー
    const OTHER_COACH_MENU: LessonMenuId = LessonMenuId::from_u128(53);

    fn menu(
        id: LessonMenuId,
        coach_id: AccountId,
        format: LessonFormat,
        is_published: bool,
    ) -> LessonMenu {
        LessonMenu::restore(
            id,
            coach_id,
            LessonMenuContent {
                sport: Sport::Tennis,
                format,
                name: LessonMenuName::new("フォア改善").unwrap(),
                description: LessonMenuDescription::default(),
            },
            is_published,
            NOW,
        )
    }

    fn menus() -> Menus {
        let sixty = LessonFormat::Live(LessonDuration::from_minutes(60).unwrap());
        CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::with_menus([
            menu(MENU, COACH, sixty, true),
            menu(UNPUBLISHED_MENU, COACH, sixty, false),
            menu(ASYNC_MENU, COACH, LessonFormat::Async, true),
            menu(OTHER_COACH_MENU, AccountId::from_u128(8), sixty, true),
        ]))
    }

    fn use_case() -> UseCase {
        PublishSlot::new(
            InMemorySlotRepository::default(),
            menus(),
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
            capacity: SlotCapacity::ONE,
            lesson_menu_id: None,
        }
    }

    #[tokio::test]
    async fn publishes_an_open_slot_and_emits_slot_published_once() {
        let use_case = use_case();

        let slot = use_case.execute(command(COACH, 10, 11)).await.unwrap();

        assert_eq!(slot.status(), SlotStatus::Open);
        assert_eq!(slot.coach_id(), COACH);
        assert_eq!(slot.capacity(), SlotCapacity::ONE);
        assert_eq!(slot.remaining_seats(), 1);
        assert_eq!(slot.lesson_menu_id(), None);
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
    async fn publishes_a_slot_with_the_given_capacity() {
        let use_case = use_case();

        let slot = use_case
            .execute(PublishSlotCommand {
                capacity: SlotCapacity::new(3).unwrap(),
                ..command(COACH, 10, 11)
            })
            .await
            .unwrap();

        assert_eq!(slot.capacity().value(), 3);
        assert_eq!(slot.remaining_seats(), 3);
        assert_eq!(use_case.slots().all(), vec![slot]);
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
                    Err(PublishSlotError::Invalid(SchedulingError::SlotOverlaps { existing }))
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
                    SchedulingError::SlotEndsBeforeItStarts
                ))
            ),
            "{result:?}"
        );
        assert!(use_case.slots().all().is_empty());
        assert!(use_case.analytics().events().is_empty());
    }

    #[tokio::test]
    async fn publishes_a_slot_bound_to_a_lesson_menu() {
        let use_case = use_case();

        let slot = use_case
            .execute(PublishSlotCommand {
                lesson_menu_id: Some(MENU),
                ..command(COACH, 10, 11)
            })
            .await
            .unwrap();

        assert_eq!(slot.lesson_menu_id(), Some(MENU));
        assert_eq!(use_case.slots().all(), vec![slot]);
        assert_eq!(use_case.analytics().events().len(), 1);
    }

    #[tokio::test]
    async fn lesson_menu_that_does_not_fit_is_rejected_without_saving_or_emitting() {
        let use_case = use_case();

        for (lesson_menu_id, to, expected) in [
            (
                OTHER_COACH_MENU,
                11,
                SchedulingError::LessonMenuOfAnotherCoach,
            ),
            (
                UNPUBLISHED_MENU,
                11,
                SchedulingError::LessonMenuNotPublished,
            ),
            (ASYNC_MENU, 11, SchedulingError::LessonMenuNotLive),
            (
                MENU,
                12,
                SchedulingError::SlotLengthMismatchesLessonMenu { minutes: 60 },
            ),
        ] {
            let result = use_case
                .execute(PublishSlotCommand {
                    lesson_menu_id: Some(lesson_menu_id),
                    ..command(COACH, 10, to)
                })
                .await;
            assert!(
                matches!(&result, Err(PublishSlotError::Invalid(error)) if *error == expected),
                "{lesson_menu_id:?}: {result:?}"
            );
        }
        let result = use_case
            .execute(PublishSlotCommand {
                lesson_menu_id: Some(MENU),
                sport: Sport::Soccer,
                ..command(COACH, 10, 11)
            })
            .await;
        assert!(
            matches!(
                result,
                Err(PublishSlotError::Invalid(
                    SchedulingError::LessonMenuSportMismatch {
                        menu: Sport::Tennis,
                        slot: Sport::Soccer
                    }
                ))
            ),
            "{result:?}"
        );
        let result = use_case
            .execute(PublishSlotCommand {
                lesson_menu_id: Some(LessonMenuId::from_u128(99)),
                ..command(COACH, 10, 11)
            })
            .await;
        assert!(
            matches!(result, Err(PublishSlotError::LessonMenuNotFound)),
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
            F: FnOnce(&mut domain::scheduling::CoachCalendar, Slot) -> Result<(), SchedulingError>
                + Send,
        {
            Err(AddToCalendarError::Unavailable(Unavailable::new(
                "connection refused",
            )))
        }

        async fn update<F>(
            &self,
            _slot_id: SlotId,
            _update: F,
        ) -> Result<Slot, crate::scheduling::UpdateSlotError>
        where
            F: FnOnce(&mut Slot) -> Result<(), SchedulingError> + Send,
        {
            Err(crate::scheduling::UpdateSlotError::Unavailable(
                Unavailable::new("connection refused"),
            ))
        }

        async fn search(&self, _criteria: &SlotCriteria) -> Result<Vec<Slot>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }

        async fn slots_of_coach(
            &self,
            _coach_id: domain::identity::AccountId,
        ) -> Result<Vec<Slot>, Unavailable> {
            Err(Unavailable::new("connection refused"))
        }
    }

    #[tokio::test]
    async fn storage_failure_is_unavailable_and_nothing_is_emitted() {
        let use_case = PublishSlot::new(
            BrokenRepository,
            menus(),
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
