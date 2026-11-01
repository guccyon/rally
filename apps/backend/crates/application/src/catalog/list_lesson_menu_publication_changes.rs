use domain::catalog::{LessonMenuId, LessonMenuPublicationChange};

use super::LessonMenuRepository;
use crate::Unavailable;

/// レッスンメニュー 1 件を本部が非公開にした・公開に戻した記録の照会のユースケース（本部だけが呼ぶ。admin-web の
/// `listLessonMenuPublicationChanges`）。誰が呼べるか（本部だけ）は境界（interface）が確かめる。照会だけで、何も保存しない。
///
/// メニューがあるかを確かめ（[`LessonMenuRepository::find`]）、あれば保存先が返す記録（新しい順。
/// [`LessonMenuRepository::publication_changes`]）をそのまま返す。ないメニューは `None`。本部が一度も非公開にしていない
/// メニューは空の一覧。コーチが自分で公開・非公開を変えたことは記録に入らない。
pub struct ListLessonMenuPublicationChanges<M> {
    menus: M,
}

impl<M> ListLessonMenuPublicationChanges<M>
where
    M: LessonMenuRepository,
{
    pub fn new(menus: M) -> Self {
        Self { menus }
    }

    pub async fn execute(
        &self,
        lesson_menu_id: LessonMenuId,
    ) -> Result<Option<Vec<LessonMenuPublicationChange>>, Unavailable> {
        if self.menus.find(lesson_menu_id).await?.is_none() {
            return Ok(None);
        }
        Ok(Some(self.menus.publication_changes(lesson_menu_id).await?))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::{
            HeadquartersUnpublishReason, LessonFormat, LessonMenu, LessonMenuContent,
            LessonMenuDescription, LessonMenuName, LessonMenuPublicationAction, Sport,
        },
        identity::AccountId,
    };

    use super::*;
    use crate::{
        catalog::{RepublishLessonMenu, UnpublishLessonMenu},
        in_memory::{InMemoryLessonMenuRepository, ManualClock},
    };

    const COACH: AccountId = AccountId::from_u128(2);
    const HEADQUARTERS: AccountId = AccountId::from_u128(1);
    const OTHER_HEADQUARTERS: AccountId = AccountId::from_u128(3);
    const MENU: LessonMenuId = LessonMenuId::from_u128(10);
    const OTHER_MENU: LessonMenuId = LessonMenuId::from_u128(11);

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
            true,
            at(5),
        )
    }

    fn reason(text: &str) -> HeadquartersUnpublishReason {
        HeadquartersUnpublishReason::new(text).unwrap()
    }

    #[tokio::test]
    async fn lists_the_changes_of_the_menu_newest_first() {
        let menus = InMemoryLessonMenuRepository::with_menus([menu(MENU), menu(OTHER_MENU)]);
        let clock = ManualClock::new(at(10));
        let unpublish = UnpublishLessonMenu::new(menus.clone(), clock.clone());
        let republish = RepublishLessonMenu::new(menus.clone(), clock.clone());
        unpublish
            .execute(MENU, HEADQUARTERS, reason("誤った記載"))
            .await
            .unwrap();
        clock.set(at(15));
        unpublish
            .execute(OTHER_MENU, HEADQUARTERS, reason("他のメニュー"))
            .await
            .unwrap();
        clock.set(at(20));
        republish.execute(MENU, OTHER_HEADQUARTERS).await.unwrap();
        // 同じ時刻なら後に保存した方が先
        unpublish
            .execute(MENU, HEADQUARTERS, reason(""))
            .await
            .unwrap();

        let changes = ListLessonMenuPublicationChanges::new(menus)
            .execute(MENU)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            changes,
            vec![
                LessonMenuPublicationChange::restore(
                    MENU,
                    LessonMenuPublicationAction::Unpublished { reason: reason("") },
                    HEADQUARTERS,
                    at(20)
                ),
                LessonMenuPublicationChange::restore(
                    MENU,
                    LessonMenuPublicationAction::Republished,
                    OTHER_HEADQUARTERS,
                    at(20)
                ),
                LessonMenuPublicationChange::restore(
                    MENU,
                    LessonMenuPublicationAction::Unpublished {
                        reason: reason("誤った記載")
                    },
                    HEADQUARTERS,
                    at(10)
                ),
            ]
        );
    }

    #[tokio::test]
    async fn a_menu_never_unpublished_by_headquarters_has_no_changes() {
        let changes =
            ListLessonMenuPublicationChanges::new(InMemoryLessonMenuRepository::with_menus([
                menu(MENU),
            ]))
            .execute(MENU)
            .await
            .unwrap();
        assert_eq!(changes, Some(Vec::new()));
    }

    #[tokio::test]
    async fn a_missing_menu_is_none() {
        let changes =
            ListLessonMenuPublicationChanges::new(InMemoryLessonMenuRepository::with_menus([
                menu(MENU),
            ]))
            .execute(LessonMenuId::from_u128(99))
            .await
            .unwrap();
        assert_eq!(changes, None);
    }
}
