use domain::{catalog::LessonMenu, identity::AccountId};

use super::LessonMenuRepository;
use crate::Unavailable;

/// コーチの自分のレッスンメニューの一覧のユースケース（admin-web の `listMyLessonMenus`。コーチだけが呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// 公開中かを問わず、自分のメニューをすべて作った時刻の古い順に返す（同じ時刻はメニューの ID の順）。なければ空。
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
        menus.sort_by_key(|menu| (menu.created_at(), menu.id()));
        Ok(menus)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::{
            LessonFormat, LessonMenuContent, LessonMenuDescription, LessonMenuId, LessonMenuName,
        },
        identity::Sport,
    };

    use super::*;
    use crate::in_memory::InMemoryLessonMenuRepository;

    const COACH: AccountId = AccountId::from_u128(2);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);

    fn menu(id: u128, coach_id: AccountId, is_published: bool, created_secs: u64) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            coach_id,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Async,
                name: LessonMenuName::new("添削").unwrap(),
                description: LessonMenuDescription::default(),
            },
            is_published,
            SystemTime::UNIX_EPOCH + Duration::from_secs(created_secs),
        )
    }

    #[tokio::test]
    async fn lists_every_menu_of_the_coach_oldest_first() {
        let use_case = ListMyLessonMenus::new(InMemoryLessonMenuRepository::with_menus([
            menu(1, COACH, true, 30),
            menu(2, COACH, false, 10),
            menu(3, OTHER_COACH, true, 20),
            // 同じ時刻は ID の順
            menu(5, COACH, true, 20),
            menu(4, COACH, true, 20),
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
            [2, 4, 5, 1].map(LessonMenuId::from_u128),
            "非公開も含め、他のコーチのメニューを含まない"
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
