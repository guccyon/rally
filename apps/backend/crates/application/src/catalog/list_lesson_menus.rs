use std::collections::BTreeSet;

use domain::{
    catalog::{LessonMenu, LessonMenuId, Sport},
    identity::AccountId,
};

use super::{
    CoachLessonMenuList, LessonMenuCoachDisplayNameLookup, LessonMenuRecentBookingCountLookup,
    LessonMenuRepository,
};
use crate::{Unavailable, clock::Clock};

/// コーチのレッスンメニューの横断の一覧のユースケース（admin-web の `listLessonMenus`。本部だけが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// すべてのコーチのメニューを、`sport` があればそのスポーツに、`is_published` があれば公開中か非公開に、`coach_id` が
/// あればそのコーチのメニューに絞って返す（いないコーチなら空の一覧）。
/// 並べ方はスポーツの順（[`Sport`] の順）→ 作ったコーチの表示名（Unicode のコードポイントの順。引けないコーチは
/// 同じスポーツの中で最後）→ コーチの ID → 作った時刻の古い順 → メニューの ID の順。コーチの表示名は profile に
/// 問い合わせて（[`LessonMenuCoachDisplayNameLookup`]）添える。メニューごとの直近の予約の数は、時計の今の時刻で scheduling に
/// 問い合わせて（[`LessonMenuRecentBookingCountLookup`]）添える（並べ方には使わない）。
pub struct ListLessonMenus<M, D, B, C> {
    menus: M,
    coaches: D,
    bookings: B,
    clock: C,
}

impl<M, D, B, C> ListLessonMenus<M, D, B, C>
where
    M: LessonMenuRepository,
    D: LessonMenuCoachDisplayNameLookup,
    B: LessonMenuRecentBookingCountLookup,
    C: Clock,
{
    pub fn new(menus: M, coaches: D, bookings: B, clock: C) -> Self {
        Self {
            menus,
            coaches,
            bookings,
            clock,
        }
    }

    pub async fn execute(
        &self,
        sport: Option<Sport>,
        is_published: Option<bool>,
        coach_id: Option<AccountId>,
    ) -> Result<CoachLessonMenuList, Unavailable> {
        let menus = match coach_id {
            Some(coach_id) => self.menus.find_by_coach(coach_id).await?,
            None => self.menus.find_all().await?,
        };
        let mut menus: Vec<LessonMenu> = menus
            .into_iter()
            .filter(|menu| {
                sport.is_none_or(|sport| menu.sport() == sport)
                    && is_published.is_none_or(|published| menu.is_published() == published)
            })
            .collect();
        let coach_ids: BTreeSet<AccountId> = menus.iter().map(LessonMenu::coach_id).collect();
        let (coaches, recent_booking_counts) = if menus.is_empty() {
            Default::default()
        } else {
            let menu_ids: BTreeSet<LessonMenuId> = menus.iter().map(LessonMenu::id).collect();
            (
                self.coaches.find_coach_display_names(&coach_ids).await?,
                self.bookings
                    .count_recent_bookings(&menu_ids, self.clock.now())
                    .await?,
            )
        };
        menus.sort_by(|a, b| {
            let name_a = coaches.get(&a.coach_id()).map(|name| name.as_str());
            let name_b = coaches.get(&b.coach_id()).map(|name| name.as_str());
            (
                a.sport(),
                name_a.is_none(),
                name_a,
                a.coach_id(),
                a.created_at(),
                a.id(),
            )
                .cmp(&(
                    b.sport(),
                    name_b.is_none(),
                    name_b,
                    b.coach_id(),
                    b.created_at(),
                    b.id(),
                ))
        });
        Ok(CoachLessonMenuList {
            lesson_menus: menus,
            coaches,
            recent_booking_counts,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        time::{Duration, SystemTime},
    };

    use domain::{
        catalog::{
            LessonDuration, LessonFormat, LessonMenuContent, LessonMenuDescription, LessonMenuId,
            LessonMenuName,
        },
        lesson::LessonType,
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryLessonMenuRepository};

    /// 表示名「たなか」のコーチ
    const TANAKA: AccountId = AccountId::from_u128(1);
    /// 表示名「いとう」のコーチ（ID は「たなか」より大きいが、表示名の順で先）
    const ITO: AccountId = AccountId::from_u128(2);
    /// 表示名「いとう」のもう 1 人のコーチ（同じ表示名は ID の順）
    const ITO_2: AccountId = AccountId::from_u128(3);
    /// 表示名が引けないコーチ
    const UNKNOWN: AccountId = AccountId::from_u128(9);

    /// 表示名を固定の表から返す偽物。問い合わせた ID を記録する。
    #[derive(Default)]
    struct FakeNames {
        asked: std::sync::Mutex<Vec<BTreeSet<AccountId>>>,
    }

    impl LessonMenuCoachDisplayNameLookup for FakeNames {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.asked.lock().unwrap().push(coach_ids.clone());
            Ok([(TANAKA, "たなか"), (ITO, "いとう"), (ITO_2, "いとう")]
                .into_iter()
                .filter(|(id, _)| coach_ids.contains(id))
                .map(|(id, name)| (id, DisplayName::new(name).unwrap()))
                .collect())
        }
    }

    /// 直近の予約の数を固定の表から返す偽物。問い合わせたメニューの ID と時刻を記録する。
    #[derive(Default)]
    struct FakeCounts {
        asked: std::sync::Mutex<Vec<(BTreeSet<LessonMenuId>, SystemTime)>>,
    }

    impl LessonMenuRecentBookingCountLookup for FakeCounts {
        async fn count_recent_bookings(
            &self,
            lesson_menu_ids: &BTreeSet<LessonMenuId>,
            now: SystemTime,
        ) -> Result<BTreeMap<LessonMenuId, u32>, Unavailable> {
            self.asked
                .lock()
                .unwrap()
                .push((lesson_menu_ids.clone(), now));
            Ok([(1, 3), (4, 1), (7, 2)]
                .into_iter()
                .map(|(id, count)| (LessonMenuId::from_u128(id), count))
                .filter(|(id, _)| lesson_menu_ids.contains(id))
                .collect())
        }
    }

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn use_case() -> ListLessonMenus<InMemoryLessonMenuRepository, FakeNames, FakeCounts, FixedClock>
    {
        ListLessonMenus::new(
            repository(),
            FakeNames::default(),
            FakeCounts::default(),
            FixedClock(now()),
        )
    }

    fn menu(
        id: u128,
        coach_id: AccountId,
        sport: Sport,
        is_published: bool,
        created_secs: u64,
    ) -> LessonMenu {
        let format = if id.is_multiple_of(2) {
            LessonFormat::Async
        } else {
            LessonFormat::new(
                LessonType::Live,
                Some(LessonDuration::from_minutes(60).unwrap()),
            )
            .unwrap()
        };
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            coach_id,
            LessonMenuContent {
                sport,
                format,
                name: LessonMenuName::new(format!("メニュー{id}")).unwrap(),
                description: LessonMenuDescription::default(),
            },
            is_published,
            SystemTime::UNIX_EPOCH + Duration::from_secs(created_secs),
        )
    }

    fn repository() -> InMemoryLessonMenuRepository {
        InMemoryLessonMenuRepository::with_menus([
            menu(1, TANAKA, Sport::Tennis, true, 10),
            menu(2, ITO, Sport::Soccer, true, 10),
            menu(3, ITO, Sport::Tennis, false, 30),
            menu(4, ITO, Sport::Tennis, true, 20),
            menu(5, UNKNOWN, Sport::Tennis, true, 0),
            menu(6, ITO_2, Sport::Tennis, true, 0),
            menu(7, TANAKA, Sport::Golf, false, 5),
            // 同じ時刻はメニューの ID の順
            menu(9, TANAKA, Sport::Tennis, true, 40),
            menu(8, TANAKA, Sport::Tennis, true, 40),
        ])
    }

    fn ids(list: &CoachLessonMenuList) -> Vec<LessonMenuId> {
        list.lesson_menus.iter().map(LessonMenu::id).collect()
    }

    #[tokio::test]
    async fn lists_every_menu_by_sport_then_coach_name_then_created_at() {
        let use_case = use_case();
        let list = use_case.execute(None, None, None).await.unwrap();
        assert_eq!(
            ids(&list),
            // テニス（いとう 2 → いとう 3 → たなか → 表示名のないコーチ）→ ゴルフ → サッカー
            [4, 3, 6, 1, 8, 9, 5, 7, 2].map(LessonMenuId::from_u128),
        );
        assert_eq!(
            list.coaches,
            BTreeMap::from([
                (TANAKA, DisplayName::new("たなか").unwrap()),
                (ITO, DisplayName::new("いとう").unwrap()),
                (ITO_2, DisplayName::new("いとう").unwrap()),
            ]),
            "表示名が引けないコーチは含めない"
        );
    }

    #[tokio::test]
    async fn filters_by_sport_and_published() {
        let use_case = use_case();
        let list = use_case
            .execute(Some(Sport::Tennis), None, None)
            .await
            .unwrap();
        assert_eq!(
            ids(&list),
            [4, 3, 6, 1, 8, 9, 5].map(LessonMenuId::from_u128)
        );
        let list = use_case.execute(None, Some(false), None).await.unwrap();
        assert_eq!(ids(&list), [3, 7].map(LessonMenuId::from_u128));
        assert_eq!(
            list.coaches.keys().copied().collect::<Vec<_>>(),
            [TANAKA, ITO],
            "絞り込んだメニューのコーチの表示名だけ"
        );
        let list = use_case
            .execute(Some(Sport::Tennis), Some(true), None)
            .await
            .unwrap();
        assert_eq!(ids(&list), [4, 6, 1, 8, 9, 5].map(LessonMenuId::from_u128));
    }

    #[tokio::test]
    async fn no_menus_is_an_empty_list_without_asking_names() {
        let use_case = use_case();
        let list = use_case
            .execute(Some(Sport::Swimming), None, None)
            .await
            .unwrap();
        assert!(list.lesson_menus.is_empty());
        assert!(list.coaches.is_empty());
        assert!(list.recent_booking_counts.is_empty());
        assert!(use_case.coaches.asked.lock().unwrap().is_empty());
        assert!(use_case.bookings.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn filters_by_coach_together_with_sport_and_published() {
        let use_case = use_case();
        let list = use_case.execute(None, None, Some(TANAKA)).await.unwrap();
        assert_eq!(
            ids(&list),
            [1, 8, 9, 7].map(LessonMenuId::from_u128),
            "公開中も非公開もまとめて、並べ方は変わらない"
        );
        assert_eq!(list.coaches.keys().copied().collect::<Vec<_>>(), [TANAKA]);
        let list = use_case
            .execute(Some(Sport::Tennis), None, Some(TANAKA))
            .await
            .unwrap();
        assert_eq!(ids(&list), [1, 8, 9].map(LessonMenuId::from_u128));
        let list = use_case
            .execute(None, Some(false), Some(ITO))
            .await
            .unwrap();
        assert_eq!(ids(&list), [3].map(LessonMenuId::from_u128));
    }

    #[tokio::test]
    async fn unknown_coach_is_an_empty_list() {
        let use_case = use_case();
        let list = use_case
            .execute(None, None, Some(AccountId::from_u128(99)))
            .await
            .unwrap();
        assert!(list.lesson_menus.is_empty());
        assert!(list.coaches.is_empty());
        assert!(list.recent_booking_counts.is_empty());
        assert!(use_case.coaches.asked.lock().unwrap().is_empty());
        assert!(use_case.bookings.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn adds_recent_booking_counts_of_the_listed_menus_at_the_current_time() {
        let use_case = use_case();
        let list = use_case
            .execute(Some(Sport::Tennis), Some(true), None)
            .await
            .unwrap();
        assert_eq!(
            ids(&list),
            [4, 6, 1, 8, 9, 5].map(LessonMenuId::from_u128),
            "直近の予約の数は並べ方を変えない"
        );
        assert_eq!(
            list.recent_booking_counts,
            BTreeMap::from([
                (LessonMenuId::from_u128(1), 3),
                (LessonMenuId::from_u128(4), 1)
            ]),
            "一覧に載ったメニューの数だけ。予約のないメニューは含まない"
        );
        assert_eq!(
            *use_case.bookings.asked.lock().unwrap(),
            [(
                [4, 6, 1, 8, 9, 5].map(LessonMenuId::from_u128).into(),
                now()
            )],
            "絞り込んだメニューだけを、時計の今の時刻で問い合わせる"
        );
    }
}
