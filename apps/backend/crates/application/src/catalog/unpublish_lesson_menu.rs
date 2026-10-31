use domain::catalog::{HeadquartersUnpublishReason, LessonMenu, LessonMenuId};

use super::{ChangeLessonMenuPublicationError, LessonMenuRepository};
use crate::clock::Clock;

/// 本部によるレッスンメニューの非公開のユースケース（admin-web の `unpublishLessonMenu`。本部だけが呼ぶ）。誰が呼べるか
/// （本部だけ）は境界（interface）が確かめる。
///
/// メニューを読み、domain の [`LessonMenu::unpublish_by_headquarters`] で非公開にして（理由と今の時刻を残す）保存し、
/// 変えたあとのメニューを返す。ここでは判定しない。拒否では何も保存しない。
/// 行動イベントも境界間イベントも発火しない（本部の運営の操作で、他のコンテキストはメニューをポートで読む）。
pub struct UnpublishLessonMenu<M, C> {
    menus: M,
    clock: C,
}

impl<M, C> UnpublishLessonMenu<M, C>
where
    M: LessonMenuRepository,
    C: Clock,
{
    pub fn new(menus: M, clock: C) -> Self {
        Self { menus, clock }
    }

    pub async fn execute(
        &self,
        lesson_menu_id: LessonMenuId,
        reason: HeadquartersUnpublishReason,
    ) -> Result<LessonMenu, ChangeLessonMenuPublicationError> {
        let mut menu = self
            .menus
            .find(lesson_menu_id)
            .await?
            .ok_or(ChangeLessonMenuPublicationError::LessonMenuNotFound)?;
        menu.unpublish_by_headquarters(reason, self.clock.now())
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
            HeadquartersUnpublish, LessonFormat, LessonMenuContent, LessonMenuDescription,
            LessonMenuName, Sport,
        },
        identity::AccountId,
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryLessonMenuRepository};

    const COACH: AccountId = AccountId::from_u128(2);
    const MENU: LessonMenuId = LessonMenuId::from_u128(10);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn menu() -> LessonMenu {
        LessonMenu::restore(
            MENU,
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

    fn use_case() -> UnpublishLessonMenu<InMemoryLessonMenuRepository, FixedClock> {
        UnpublishLessonMenu::new(
            InMemoryLessonMenuRepository::with_menus([menu()]),
            FixedClock(at(100)),
        )
    }

    #[tokio::test]
    async fn headquarters_unpublishes_a_menu_with_the_reason_and_now() {
        let use_case = use_case();
        let menu = use_case
            .execute(MENU, reason("誤解を招く表現"))
            .await
            .unwrap();
        assert!(!menu.is_published());
        assert_eq!(
            menu.headquarters_unpublish(),
            Some(&HeadquartersUnpublish::new(
                reason("誤解を招く表現"),
                at(100)
            ))
        );
        assert_eq!(use_case.menus().all(), vec![menu]);
    }

    #[tokio::test]
    async fn unpublishing_twice_is_rejected_and_keeps_the_first_record() {
        let use_case = use_case();
        let first = use_case.execute(MENU, reason("最初")).await.unwrap();
        let result = use_case.execute(MENU, reason("二度目")).await;
        assert!(
            matches!(
                result,
                Err(ChangeLessonMenuPublicationError::Invalid(
                    DomainError::LessonMenuAlreadyUnpublishedByHeadquarters
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.menus().all(), vec![first]);
    }

    #[tokio::test]
    async fn a_missing_menu_is_not_found_and_nothing_changes() {
        let use_case = use_case();
        let result = use_case
            .execute(LessonMenuId::from_u128(99), reason(""))
            .await;
        assert!(
            matches!(
                result,
                Err(ChangeLessonMenuPublicationError::LessonMenuNotFound)
            ),
            "{result:?}"
        );
        assert_eq!(use_case.menus().all(), vec![menu()]);
    }
}
