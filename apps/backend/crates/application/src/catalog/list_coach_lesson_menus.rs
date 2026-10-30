use domain::{
    catalog::LessonMenu,
    identity::{AccountId, Sport},
};

use super::{LessonMenuRepository, lesson_menu_listing::sort_for_listing};
use crate::Unavailable;

/// コーチの公開中のレッスンメニューの一覧のユースケース（student-app の `listCoachLessonMenus`。生徒がコーチを選ぶ
/// 手がかり）。照会だけで、何も保存しない。
///
/// コーチ `coach_id` のメニューのうち公開中のものだけを、`sport` を渡せばそのスポーツに絞って、作った時刻の古い順
/// （同じ時刻は ID の順）で返す。非公開のメニューは返さない。メニューがなければ（コーチでない ID・ない ID を含む）空。
/// どのクライアントがどのスポーツを扱うか（student-app はゴルフを扱わない）は境界（interface）が決める。
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
            .filter(|menu| menu.is_published())
            .filter(|menu| sport.is_none_or(|sport| menu.sport() == sport))
            .collect();
        sort_for_listing(&mut menus);
        Ok(menus)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::{LessonMenuContent, LessonMenuDescription, LessonMenuId, LessonMenuName},
        lesson::LessonType,
    };

    use super::*;
    use crate::in_memory::InMemoryLessonMenuRepository;

    const COACH: AccountId = AccountId::from_u128(1);
    const OTHER_COACH: AccountId = AccountId::from_u128(2);

    fn menu(
        id: u128,
        coach_id: AccountId,
        sport: Sport,
        created_secs: u64,
        is_published: bool,
    ) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            coach_id,
            sport,
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

    fn use_case() -> ListCoachLessonMenus<InMemoryLessonMenuRepository> {
        ListCoachLessonMenus::new(InMemoryLessonMenuRepository::with_menus([
            menu(1, COACH, Sport::Tennis, 30, true),
            menu(2, COACH, Sport::Golf, 10, true),
            menu(3, COACH, Sport::Tennis, 20, false),
            menu(4, COACH, Sport::Soccer, 5, true),
            menu(5, OTHER_COACH, Sport::Tennis, 1, true),
        ]))
    }

    async fn ids(
        use_case: &ListCoachLessonMenus<InMemoryLessonMenuRepository>,
        coach_id: AccountId,
        sport: Option<Sport>,
    ) -> Vec<u128> {
        use_case
            .execute(coach_id, sport)
            .await
            .unwrap()
            .iter()
            .map(|menu| menu.id().as_u128())
            .collect()
    }

    #[tokio::test]
    async fn lists_only_the_published_menus_of_the_coach_oldest_first() {
        let use_case = use_case();
        assert_eq!(
            ids(&use_case, COACH, None).await,
            [4, 2, 1],
            "非公開と他のコーチのメニューは含まない"
        );
        assert!(
            ids(&use_case, AccountId::from_u128(99), None)
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn narrows_down_to_the_sport() {
        let use_case = use_case();
        assert_eq!(ids(&use_case, COACH, Some(Sport::Tennis)).await, [1]);
        assert_eq!(ids(&use_case, COACH, Some(Sport::Soccer)).await, [4]);
        assert!(
            ids(&use_case, COACH, Some(Sport::Swimming))
                .await
                .is_empty()
        );
    }
}
