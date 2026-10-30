use domain::{
    catalog::LessonMenu,
    identity::{AccountId, Sport},
};

use super::LessonMenuRepository;
use crate::Unavailable;

/// コーチの公開中のレッスンメニューの一覧のユースケース（student-app の `listCoachLessonMenus`。生徒が呼ぶ）。
/// 照会だけで、何も保存しない。
///
/// コーチ `coach_id` の公開中のメニューだけを、`sport` があればそのスポーツに絞って、作った時刻の古い順に返す
/// （同じ時刻はメニューの ID の順）。なければ空。どのスポーツをどのクライアントに見せるか（student-app はゴルフを
/// 扱わない）は境界が決める。
pub struct ListCoachLessonMenus<M> {
    menus: M,
}

impl<M> ListCoachLessonMenus<M>
where
    M: LessonMenuRepository,
{
    pub fn new(menus: M) -> Self {
        Self { menus }
    }

    pub async fn execute(
        &self,
        coach_id: AccountId,
        sport: Option<Sport>,
    ) -> Result<Vec<LessonMenu>, Unavailable> {
        let mut menus: Vec<LessonMenu> = self
            .menus
            .find_by_coach(coach_id)
            .await?
            .into_iter()
            .filter(|menu| menu.is_published() && sport.is_none_or(|sport| menu.sport() == sport))
            .collect();
        menus.sort_by_key(|menu| (menu.created_at(), menu.id()));
        Ok(menus)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::catalog::{
        LessonFormat, LessonMenuContent, LessonMenuDescription, LessonMenuId, LessonMenuName,
    };

    use super::*;
    use crate::in_memory::InMemoryLessonMenuRepository;

    const COACH: AccountId = AccountId::from_u128(2);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);

    fn menu(
        id: u128,
        coach_id: AccountId,
        sport: Sport,
        is_published: bool,
        created_secs: u64,
    ) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            coach_id,
            LessonMenuContent {
                sport,
                format: LessonFormat::Async,
                name: LessonMenuName::new("添削").unwrap(),
                description: LessonMenuDescription::default(),
            },
            is_published,
            SystemTime::UNIX_EPOCH + Duration::from_secs(created_secs),
        )
    }

    fn use_case() -> ListCoachLessonMenus<InMemoryLessonMenuRepository> {
        ListCoachLessonMenus::new(InMemoryLessonMenuRepository::with_menus([
            menu(1, COACH, Sport::Tennis, true, 30),
            menu(2, COACH, Sport::Soccer, true, 10),
            menu(3, COACH, Sport::Tennis, false, 5),
            menu(4, OTHER_COACH, Sport::Tennis, true, 1),
            menu(5, COACH, Sport::Tennis, true, 20),
        ]))
    }

    async fn ids(sport: Option<Sport>) -> Vec<LessonMenuId> {
        use_case()
            .execute(COACH, sport)
            .await
            .unwrap()
            .iter()
            .map(LessonMenu::id)
            .collect()
    }

    #[tokio::test]
    async fn lists_only_the_published_menus_of_the_coach_oldest_first() {
        assert_eq!(ids(None).await, [2, 5, 1].map(LessonMenuId::from_u128));
    }

    #[tokio::test]
    async fn filters_by_sport() {
        assert_eq!(
            ids(Some(Sport::Tennis)).await,
            [5, 1].map(LessonMenuId::from_u128)
        );
        assert!(ids(Some(Sport::Swimming)).await.is_empty());
    }
}
