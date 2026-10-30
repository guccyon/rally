use domain::{catalog::LessonMenu, identity::AccountId};

use super::{LessonMenuRepository, lesson_menu_listing::sort_for_listing};
use crate::Unavailable;

/// コーチの自分のレッスンメニューの一覧のユースケース（admin-web の `listMyLessonMenus`。コーチだけが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// 公開中も非公開もすべて、作った時刻の古い順（同じ時刻は ID の順）で返す。メニューがなければ空。
/// 誰が呼べるか（コーチだけ）は境界（interface）が確かめ、`coach_id` は認証したアカウント。
pub struct ListMyLessonMenus<M> {
    menus: M,
}

impl<M> ListMyLessonMenus<M>
where
    M: LessonMenuRepository,
{
    pub fn new(menus: M) -> Self {
        Self { menus }
    }

    pub async fn execute(&self, coach_id: AccountId) -> Result<Vec<LessonMenu>, Unavailable> {
        let mut menus = self.menus.find_by_coach(coach_id).await?;
        sort_for_listing(&mut menus);
        Ok(menus)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::{LessonMenuContent, LessonMenuDescription, LessonMenuId, LessonMenuName},
        identity::Sport,
        lesson::LessonType,
    };

    use super::*;
    use crate::in_memory::InMemoryLessonMenuRepository;

    const COACH: AccountId = AccountId::from_u128(1);
    const OTHER_COACH: AccountId = AccountId::from_u128(2);

    fn menu(id: u128, coach_id: AccountId, created_secs: u64, is_published: bool) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            coach_id,
            Sport::Tennis,
            LessonType::Async,
            LessonMenuContent {
                name: LessonMenuName::new("添削").unwrap(),
                description: LessonMenuDescription::default(),
                duration: None,
                is_published,
            },
            SystemTime::UNIX_EPOCH + Duration::from_secs(created_secs),
            SystemTime::UNIX_EPOCH + Duration::from_secs(created_secs),
        )
    }

    #[tokio::test]
    async fn lists_every_menu_of_the_coach_oldest_first() {
        let use_case = ListMyLessonMenus::new(InMemoryLessonMenuRepository::with_menus([
            menu(1, COACH, 30, true),
            menu(2, OTHER_COACH, 10, true),
            menu(3, COACH, 10, false),
            // 同じ時刻は ID の順
            menu(5, COACH, 20, true),
            menu(4, COACH, 20, true),
        ]));

        let ids: Vec<LessonMenuId> = use_case
            .execute(COACH)
            .await
            .unwrap()
            .iter()
            .map(LessonMenu::id)
            .collect();
        assert_eq!(
            ids,
            [3, 4, 5, 1].map(LessonMenuId::from_u128),
            "非公開も含み、他のコーチのメニューは含まない"
        );
        assert!(
            use_case
                .execute(AccountId::from_u128(99))
                .await
                .unwrap()
                .is_empty()
        );
    }
}
