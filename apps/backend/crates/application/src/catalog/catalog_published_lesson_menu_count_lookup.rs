use std::collections::{BTreeMap, BTreeSet};

use domain::{catalog::Sport, identity::AccountId};

use super::LessonMenuRepository;
use crate::{Unavailable, matching::PublishedLessonMenuCountLookup};

/// matching の [`PublishedLessonMenuCountLookup`] の catalog による実装。レッスンメニューの保存先から読んで
/// 数えるだけで、何も変えない。
#[derive(Debug, Clone)]
pub struct CatalogPublishedLessonMenuCountLookup<M> {
    menus: M,
}

impl<M> CatalogPublishedLessonMenuCountLookup<M> {
    pub fn new(menus: M) -> Self {
        Self { menus }
    }
}

impl<M> PublishedLessonMenuCountLookup for CatalogPublishedLessonMenuCountLookup<M>
where
    M: LessonMenuRepository,
{
    async fn count_published_lesson_menus(
        &self,
        coach_ids: &BTreeSet<AccountId>,
        sport: Sport,
    ) -> Result<BTreeMap<AccountId, u32>, Unavailable> {
        let mut counts = BTreeMap::new();
        for &coach_id in coach_ids {
            let count = self
                .menus
                .find_by_coach(coach_id)
                .await?
                .iter()
                .filter(|menu| menu.is_published() && menu.sport() == sport)
                .count();
            if count > 0 {
                counts.insert(coach_id, u32::try_from(count).unwrap_or(u32::MAX));
            }
        }
        Ok(counts)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::catalog::{
        LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuId,
        LessonMenuName,
    };

    use super::*;
    use crate::in_memory::InMemoryLessonMenuRepository;

    fn menu(id: u128, coach_id: u128, sport: Sport, is_published: bool) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            AccountId::from_u128(coach_id),
            LessonMenuContent {
                sport,
                format: LessonFormat::Async,
                name: LessonMenuName::new("添削").unwrap(),
                description: LessonMenuDescription::default(),
            },
            is_published,
            SystemTime::UNIX_EPOCH,
        )
    }

    #[tokio::test]
    async fn counts_published_menus_of_the_sport_per_coach() {
        let lookup =
            CatalogPublishedLessonMenuCountLookup::new(InMemoryLessonMenuRepository::with_menus([
                menu(1, 2, Sport::Tennis, true),
                menu(2, 2, Sport::Tennis, true),
                menu(3, 2, Sport::Tennis, false),
                menu(4, 2, Sport::Soccer, true),
                menu(5, 3, Sport::Soccer, true),
                menu(6, 4, Sport::Tennis, true),
            ]));

        let ids = [2, 3].map(AccountId::from_u128).into();
        let counts = lookup
            .count_published_lesson_menus(&ids, Sport::Tennis)
            .await
            .unwrap();
        assert_eq!(
            counts,
            BTreeMap::from([(AccountId::from_u128(2), 2)]),
            "非公開・他のスポーツは数えない。メニューのないコーチ（3）と聞いていないコーチ（4）は含めない"
        );
    }
}
