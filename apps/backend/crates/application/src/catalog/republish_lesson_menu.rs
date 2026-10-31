use domain::catalog::{LessonMenu, LessonMenuId};

use super::{ChangeLessonMenuPublicationError, LessonMenuRepository};

/// 本部が非公開にしたレッスンメニューを公開に戻すユースケース（admin-web の `republishLessonMenu`。本部だけが呼ぶ）。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
///
/// メニューを読み、domain の [`LessonMenu::republish_by_headquarters`] で公開中に戻して（本部が非公開にしたことと理由を
/// 消す）保存し、変えたあとのメニューを返す。ここでは判定しない。拒否では何も保存しない。
/// 行動イベントも境界間イベントも発火しない（[`super::UnpublishLessonMenu`] と同じ）。
pub struct RepublishLessonMenu<M> {
    menus: M,
}

impl<M> RepublishLessonMenu<M>
where
    M: LessonMenuRepository,
{
    pub fn new(menus: M) -> Self {
        Self { menus }
    }

    pub async fn execute(
        &self,
        lesson_menu_id: LessonMenuId,
    ) -> Result<LessonMenu, ChangeLessonMenuPublicationError> {
        let mut menu = self
            .menus
            .find(lesson_menu_id)
            .await?
            .ok_or(ChangeLessonMenuPublicationError::LessonMenuNotFound)?;
        menu.republish_by_headquarters()
            .map_err(ChangeLessonMenuPublicationError::Invalid)?;
        self.menus.update(menu.clone()).await?;
        Ok(menu)
    }

    pub fn menus(&self) -> &M {
        &self.menus
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        DomainError,
        catalog::{
            HeadquartersUnpublish, HeadquartersUnpublishReason, LessonFormat, LessonMenuContent,
            LessonMenuDescription, LessonMenuName, Sport,
        },
        identity::AccountId,
    };

    use super::*;
    use crate::in_memory::InMemoryLessonMenuRepository;

    const COACH: AccountId = AccountId::from_u128(2);
    const UNPUBLISHED_BY_HQ: LessonMenuId = LessonMenuId::from_u128(10);
    const UNPUBLISHED_BY_COACH: LessonMenuId = LessonMenuId::from_u128(11);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn menu(id: LessonMenuId) -> LessonMenu {
        LessonMenu::restore(
            id,
            COACH,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Async,
                name: LessonMenuName::new("フォーム添削").unwrap(),
                description: LessonMenuDescription::default(),
            },
            false,
            at(5),
        )
    }

    fn menus() -> Vec<LessonMenu> {
        vec![
            menu(UNPUBLISHED_BY_HQ).with_headquarters_unpublish(Some(HeadquartersUnpublish::new(
                HeadquartersUnpublishReason::new("誤った記載").unwrap(),
                at(50),
            ))),
            menu(UNPUBLISHED_BY_COACH),
        ]
    }

    fn use_case() -> RepublishLessonMenu<InMemoryLessonMenuRepository> {
        RepublishLessonMenu::new(InMemoryLessonMenuRepository::with_menus(menus()))
    }

    #[tokio::test]
    async fn headquarters_republishes_a_menu_it_unpublished() {
        let use_case = use_case();
        let menu = use_case.execute(UNPUBLISHED_BY_HQ).await.unwrap();
        assert!(menu.is_published());
        assert_eq!(menu.headquarters_unpublish(), None);
        assert_eq!(
            use_case.menus().all(),
            vec![menu, menus()[1].clone()],
            "他のメニューは変えない"
        );
    }

    #[tokio::test]
    async fn a_menu_headquarters_did_not_unpublish_is_rejected_without_change() {
        let use_case = use_case();
        let result = use_case.execute(UNPUBLISHED_BY_COACH).await;
        assert!(
            matches!(
                result,
                Err(ChangeLessonMenuPublicationError::Invalid(
                    DomainError::LessonMenuNotUnpublishedByHeadquarters
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.menus().all(), menus());
    }

    #[tokio::test]
    async fn a_missing_menu_is_not_found() {
        let use_case = use_case();
        let result = use_case.execute(LessonMenuId::from_u128(99)).await;
        assert!(
            matches!(
                result,
                Err(ChangeLessonMenuPublicationError::LessonMenuNotFound)
            ),
            "{result:?}"
        );
        assert_eq!(use_case.menus().all(), menus());
    }
}
