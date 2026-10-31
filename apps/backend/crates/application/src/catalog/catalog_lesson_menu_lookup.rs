use std::collections::{BTreeMap, BTreeSet};

use domain::catalog::{LessonMenu, LessonMenuId};

use super::LessonMenuRepository;
use crate::{Unavailable, async_lesson::VideoLessonMenuLookup, scheduling::LessonMenuLookup};

/// scheduling の [`LessonMenuLookup`] と async-lesson の [`VideoLessonMenuLookup`] の catalog による実装。
/// レッスンメニューの保存先から読むだけで、何も変えない。
#[derive(Debug, Clone)]
pub struct CatalogLessonMenuLookup<M> {
    menus: M,
}

impl<M> CatalogLessonMenuLookup<M> {
    pub fn new(menus: M) -> Self {
        Self { menus }
    }
}

impl<M> LessonMenuLookup for CatalogLessonMenuLookup<M>
where
    M: LessonMenuRepository,
{
    async fn find_lesson_menus(
        &self,
        lesson_menu_ids: &BTreeSet<LessonMenuId>,
    ) -> Result<BTreeMap<LessonMenuId, LessonMenu>, Unavailable> {
        let mut menus = BTreeMap::new();
        for &id in lesson_menu_ids {
            if let Some(menu) = self.menus.find(id).await? {
                menus.insert(id, menu);
            }
        }
        Ok(menus)
    }
}

impl<M> VideoLessonMenuLookup for CatalogLessonMenuLookup<M>
where
    M: LessonMenuRepository,
{
    async fn find_lesson_menu(
        &self,
        lesson_menu_id: LessonMenuId,
    ) -> Result<Option<LessonMenu>, Unavailable> {
        self.menus.find(lesson_menu_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        catalog::Sport,
        catalog::{LessonFormat, LessonMenuContent, LessonMenuDescription, LessonMenuName},
        identity::AccountId,
    };

    use super::*;
    use crate::in_memory::InMemoryLessonMenuRepository;

    fn menu(id: u128, is_published: bool) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(id),
            AccountId::from_u128(7),
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Async,
                name: LessonMenuName::new("添削").unwrap(),
                description: LessonMenuDescription::default(),
            },
            is_published,
            SystemTime::UNIX_EPOCH,
        )
    }

    #[tokio::test]
    async fn finds_the_menus_of_the_ids_whether_published_or_not() {
        let lookup = CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::with_menus([
            menu(1, true),
            menu(2, false),
            menu(3, true),
        ]));
        let found = lookup
            .find_lesson_menus(&BTreeSet::from([1, 2, 9].map(LessonMenuId::from_u128)))
            .await
            .unwrap();
        assert_eq!(
            found,
            BTreeMap::from([
                (LessonMenuId::from_u128(1), menu(1, true)),
                (LessonMenuId::from_u128(2), menu(2, false)),
            ]),
            "ない ID（9）は含めない"
        );
        assert!(
            lookup
                .find_lesson_menus(&BTreeSet::new())
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn finds_a_menu_for_a_video_whether_published_or_not() {
        let lookup = CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::with_menus([
            menu(1, true),
            menu(2, false),
        ]));
        let find =
            |id| VideoLessonMenuLookup::find_lesson_menu(&lookup, LessonMenuId::from_u128(id));
        assert_eq!(find(1).await.unwrap(), Some(menu(1, true)));
        assert_eq!(find(2).await.unwrap(), Some(menu(2, false)));
        assert_eq!(find(9).await.unwrap(), None);
    }
}
