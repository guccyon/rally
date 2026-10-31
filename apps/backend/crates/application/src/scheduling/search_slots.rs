use std::{collections::BTreeSet, num::NonZeroUsize, time::SystemTime};

use domain::{
    catalog::LessonMenuId,
    identity::{AccountId, Sport},
};

use super::{
    CoachIntroductionLookup, LessonMenuLookup, SlotCriteria, SlotCursor, SlotPage, SlotRepository,
};
use crate::{Unavailable, clock::Clock};

/// スロットの検索の入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSlotsQuery {
    pub sport: Sport,
    /// あればこのコーチの枠だけ
    pub coach_id: Option<AccountId>,
    /// この時刻以降に始まる枠（含む）。省略時は現在時刻
    pub starts_after: Option<SystemTime>,
    /// あればこの時刻より前に始まる枠（含まない）
    pub starts_before: Option<SystemTime>,
    /// 1 ページの最大件数
    pub limit: NonZeroUsize,
    /// 前のページの `next_cursor`
    pub cursor: Option<SlotCursor>,
}

/// スロットの検索のユースケース（student-app の `searchSlots`）。
///
/// 公開中でまだ予約されていない枠を、スポーツ・コーチ・開始時刻の期間で絞り、
/// （開始時刻, スロット ID）の昇順でページに分けて返す。ページに出るコーチの紹介（表示名と自己紹介文）は
/// profile に問い合わせて（[`CoachIntroductionLookup`]）添える。ページの枠に結びついたレッスンメニュー（名前と時間）は
/// catalog に問い合わせて（[`LessonMenuLookup`]）、公開中のものだけを添える。
pub struct SearchSlots<S, M, I, C> {
    slots: S,
    lesson_menus: M,
    coaches: I,
    clock: C,
}

impl<S, M, I, C> SearchSlots<S, M, I, C>
where
    S: SlotRepository,
    M: LessonMenuLookup,
    I: CoachIntroductionLookup,
    C: Clock,
{
    pub fn new(slots: S, lesson_menus: M, coaches: I, clock: C) -> Self {
        Self {
            slots,
            lesson_menus,
            coaches,
            clock,
        }
    }

    pub async fn execute(&self, query: SearchSlotsQuery) -> Result<SlotPage, Unavailable> {
        let limit = query.limit.get();
        // 1 件多く読み、あふれたら次のページがある
        let mut items = self
            .slots
            .search(&SlotCriteria {
                sport: query.sport,
                coach_id: query.coach_id,
                starts_after: query.starts_after.unwrap_or_else(|| self.clock.now()),
                starts_before: query.starts_before,
                after: query.cursor,
                limit: limit + 1,
            })
            .await?;
        let next_cursor = if items.len() > limit {
            items.truncate(limit);
            items.last().map(SlotCursor::after)
        } else {
            None
        };
        let coach_ids: BTreeSet<AccountId> = items.iter().map(|slot| slot.coach_id()).collect();
        let coaches = if coach_ids.is_empty() {
            Default::default()
        } else {
            self.coaches.find_coach_introductions(&coach_ids).await?
        };
        let lesson_menu_ids: BTreeSet<LessonMenuId> = items
            .iter()
            .filter_map(|slot| slot.lesson_menu_id())
            .collect();
        let lesson_menus = if lesson_menu_ids.is_empty() {
            Default::default()
        } else {
            let mut menus = self
                .lesson_menus
                .find_lesson_menus(&lesson_menu_ids)
                .await?;
            menus.retain(|_, menu| menu.is_published());
            menus
        };
        Ok(SlotPage {
            items,
            next_cursor,
            coaches,
            lesson_menus,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use domain::{
        catalog::{
            LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
            LessonMenuName,
        },
        profile::{CoachBio, DisplayName},
        scheduling::{Slot, SlotId, SlotStatus},
    };

    use super::*;
    use crate::{
        catalog::CatalogLessonMenuLookup,
        in_memory::{FixedClock, InMemoryLessonMenuRepository, InMemorySlotRepository},
        scheduling::CoachIntroduction,
    };

    type Menus = CatalogLessonMenuLookup<InMemoryLessonMenuRepository>;

    fn menu(id: u128, is_published: bool) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            COACH,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Live(LessonDuration::from_minutes(60).unwrap()),
                name: LessonMenuName::new("フォア改善").unwrap(),
                description: LessonMenuDescription::default(),
            },
            is_published,
            SystemTime::UNIX_EPOCH,
        )
    }

    fn menus(menus: impl IntoIterator<Item = LessonMenu>) -> Menus {
        CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::with_menus(menus))
    }

    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn slot(id: u128, coach: AccountId, sport: Sport, hour: u64, status: SlotStatus) -> Slot {
        // 予約済みの枠は確定した予約 1 件で埋まっている
        let confirmed = u32::from(status == SlotStatus::Booked);
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            sport,
            at(hour),
            at(hour + 1),
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

    fn query(limit: usize) -> SearchSlotsQuery {
        SearchSlotsQuery {
            sport: Sport::Tennis,
            coach_id: None,
            starts_after: None,
            starts_before: None,
            limit: NonZeroUsize::new(limit).unwrap(),
            cursor: None,
        }
    }

    fn ids(page: &SlotPage) -> Vec<u128> {
        page.items.iter().map(|s| s.id().as_u128()).collect()
    }

    fn introduction() -> CoachIntroduction {
        CoachIntroduction {
            display_name: DisplayName::new("たなか").unwrap(),
            bio: CoachBio::new("テニス歴 20 年").unwrap(),
        }
    }

    /// `COACH` の紹介だけを知っていて（`OTHER_COACH` は紹介が引けない）、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl RecordingLookup {
        fn asked(&self) -> Vec<BTreeSet<AccountId>> {
            self.asked.lock().unwrap().clone()
        }
    }

    impl CoachIntroductionLookup for RecordingLookup {
        async fn find_coach_introductions(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, CoachIntroduction>, Unavailable> {
            self.asked.lock().unwrap().push(coach_ids.clone());
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| (*id, introduction()))
                .collect())
        }
    }

    /// 現在時刻は 10 時。テニスの公開中の枠は 11・12・13・14 時（14 時は別のコーチ）。
    fn use_case() -> SearchSlots<InMemorySlotRepository, Menus, RecordingLookup, FixedClock> {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, Sport::Tennis, 9, SlotStatus::Open),
            slot(2, COACH, Sport::Tennis, 13, SlotStatus::Open),
            slot(3, COACH, Sport::Tennis, 11, SlotStatus::Open),
            slot(4, COACH, Sport::Soccer, 12, SlotStatus::Open),
            slot(5, COACH, Sport::Tennis, 12, SlotStatus::Open),
            slot(6, COACH, Sport::Tennis, 15, SlotStatus::Booked),
            slot(7, COACH, Sport::Tennis, 16, SlotStatus::Closed),
            slot(8, OTHER_COACH, Sport::Tennis, 14, SlotStatus::Open),
        ]);
        SearchSlots::new(
            slots,
            menus([]),
            RecordingLookup::default(),
            FixedClock(at(10)),
        )
    }

    #[tokio::test]
    async fn returns_open_slots_of_the_sport_from_now_in_start_order() {
        let page = use_case().execute(query(20)).await.unwrap();
        assert_eq!(ids(&page), [3, 5, 2, 8]);
        assert_eq!(page.next_cursor, None);
    }

    #[tokio::test]
    async fn filters_by_coach_and_period() {
        let use_case = use_case();
        let page = use_case
            .execute(SearchSlotsQuery {
                coach_id: Some(OTHER_COACH),
                ..query(20)
            })
            .await
            .unwrap();
        assert_eq!(ids(&page), [8]);

        let page = use_case
            .execute(SearchSlotsQuery {
                starts_after: Some(at(9)),
                starts_before: Some(at(13)),
                ..query(20)
            })
            .await
            .unwrap();
        assert_eq!(ids(&page), [1, 3, 5], "開始は含み、終わりは含まない");

        let page = use_case
            .execute(SearchSlotsQuery {
                sport: Sport::Soccer,
                ..query(20)
            })
            .await
            .unwrap();
        assert_eq!(ids(&page), [4]);
    }

    #[tokio::test]
    async fn pages_through_results_with_the_cursor() {
        let use_case = use_case();
        let mut cursor = None;
        let mut pages = Vec::new();
        loop {
            let page = use_case
                .execute(SearchSlotsQuery { cursor, ..query(3) })
                .await
                .unwrap();
            pages.push(ids(&page));
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        assert_eq!(pages, [vec![3, 5, 2], vec![8]]);

        // ちょうど割り切れるときは、最後のページに次のカーソルがない
        let page = use_case.execute(query(4)).await.unwrap();
        assert_eq!((ids(&page).len(), page.next_cursor), (4, None));
    }

    #[tokio::test]
    async fn slots_starting_at_the_same_time_are_ordered_by_id() {
        let slots = InMemorySlotRepository::with_slots([
            slot(9, OTHER_COACH, Sport::Tennis, 11, SlotStatus::Open),
            slot(3, COACH, Sport::Tennis, 11, SlotStatus::Open),
        ]);
        let use_case = SearchSlots::new(
            slots,
            menus([]),
            RecordingLookup::default(),
            FixedClock(at(10)),
        );
        let first = use_case.execute(query(1)).await.unwrap();
        assert_eq!(ids(&first), [3]);
        let second = use_case
            .execute(SearchSlotsQuery {
                cursor: first.next_cursor,
                ..query(1)
            })
            .await
            .unwrap();
        assert_eq!(ids(&second), [9]);
        assert_eq!(second.next_cursor, None);
    }

    #[tokio::test]
    async fn adds_introductions_of_the_coaches_on_the_page() {
        let use_case = use_case();
        let page = use_case.execute(query(20)).await.unwrap();
        assert_eq!(ids(&page), [3, 5, 2, 8]);
        assert_eq!(
            page.coaches,
            BTreeMap::from([(COACH, introduction())]),
            "紹介が引けないコーチ（OTHER_COACH）は含まない"
        );
        assert_eq!(
            use_case.coaches.asked(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "ページのコーチを重ねずに 1 回で問い合わせる"
        );

        // ページに出ないコーチは問い合わせない
        let page = use_case.execute(query(2)).await.unwrap();
        assert_eq!(ids(&page), [3, 5]);
        assert_eq!(
            use_case.coaches.asked().last(),
            Some(&BTreeSet::from([COACH]))
        );
    }

    #[tokio::test]
    async fn empty_page_does_not_ask_for_introductions() {
        let use_case = use_case();
        let page = use_case
            .execute(SearchSlotsQuery {
                sport: Sport::Baseball,
                ..query(20)
            })
            .await
            .unwrap();
        assert!(page.items.is_empty());
        assert!(page.coaches.is_empty());
        assert!(use_case.coaches.asked().is_empty());
    }

    #[tokio::test]
    async fn adds_only_the_published_lesson_menus_bound_to_the_slots_on_the_page() {
        let bound = |id, hour, menu: u128| {
            slot(id, COACH, Sport::Tennis, hour, SlotStatus::Open)
                .with_restored_lesson_menu(Some(LessonMenuId::from_u128(menu)))
        };
        let slots = InMemorySlotRepository::with_slots([
            bound(1, 11, 50),
            bound(2, 12, 51),
            // 消えたメニュー（catalog にない）
            bound(3, 13, 52),
            slot(4, COACH, Sport::Tennis, 14, SlotStatus::Open),
        ]);
        let use_case = SearchSlots::new(
            slots,
            menus([menu(50, true), menu(51, false), menu(53, true)]),
            RecordingLookup::default(),
            FixedClock(at(10)),
        );
        let page = use_case.execute(query(20)).await.unwrap();
        assert_eq!(ids(&page), [1, 2, 3, 4]);
        assert_eq!(
            page.items[0].lesson_menu_id(),
            Some(LessonMenuId::from_u128(50))
        );
        assert_eq!(
            page.lesson_menus,
            BTreeMap::from([(LessonMenuId::from_u128(50), menu(50, true))]),
            "非公開（51）・ない（52）メニューと、ページの枠に結びついていないメニュー（53）は含めない"
        );

        let page = use_case
            .execute(SearchSlotsQuery {
                starts_after: Some(at(14)),
                ..query(20)
            })
            .await
            .unwrap();
        assert_eq!(ids(&page), [4]);
        assert!(page.lesson_menus.is_empty());
    }
}
