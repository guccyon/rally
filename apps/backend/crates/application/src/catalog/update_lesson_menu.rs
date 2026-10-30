use domain::{
    DomainError,
    catalog::{LessonMenu, LessonMenuContent, LessonMenuId},
    identity::AccountId,
};

use super::LessonMenuRepository;
use crate::{Unavailable, clock::Clock};

/// レッスンメニューの変更の入力。`account_id` は認証したアカウント（メニューのコーチかは domain が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateLessonMenuCommand {
    pub account_id: AccountId,
    pub lesson_menu_id: LessonMenuId,
    pub content: LessonMenuContent,
}

#[derive(Debug)]
pub enum UpdateLessonMenuError {
    /// メニューがない
    LessonMenuNotFound,
    /// 変更がドメインの不変条件を満たさない（メニューのコーチでない、1 回の時間がレッスンの種類に合わない）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// レッスンメニューの変更のユースケース（admin-web の `updateLessonMenu`。コーチだけが呼ぶ）。
///
/// 名前・説明・1 回の時間・公開中かを丸ごと置き換える（非公開にするのもこれ）。メニューを読み、domain の
/// [`LessonMenu::revise`] で直して保存する。行動イベントも境界間イベントも発火しない。
pub struct UpdateLessonMenu<M, C> {
    menus: M,
    clock: C,
}

impl<M, C> UpdateLessonMenu<M, C>
where
    M: LessonMenuRepository,
    C: Clock,
{
    pub fn new(menus: M, clock: C) -> Self {
        Self { menus, clock }
    }

    pub async fn execute(
        &self,
        command: UpdateLessonMenuCommand,
    ) -> Result<LessonMenu, UpdateLessonMenuError> {
        let mut menu = self
            .menus
            .find(command.lesson_menu_id)
            .await
            .map_err(UpdateLessonMenuError::Unavailable)?
            .ok_or(UpdateLessonMenuError::LessonMenuNotFound)?;
        menu.revise(command.account_id, command.content, self.clock.now())
            .map_err(UpdateLessonMenuError::Invalid)?;
        self.menus
            .update(menu.clone())
            .await
            .map_err(UpdateLessonMenuError::Unavailable)?;
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
        catalog::{LessonDuration, LessonMenuDescription, LessonMenuName},
        identity::{Coach, Sport},
        lesson::LessonType,
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryLessonMenuRepository};

    const COACH: AccountId = AccountId::from_u128(1);
    const OTHER_COACH: AccountId = AccountId::from_u128(2);
    const STUDENT: AccountId = AccountId::from_u128(3);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn content(name: &str, duration: Option<i64>, is_published: bool) -> LessonMenuContent {
        LessonMenuContent {
            name: LessonMenuName::new(name).unwrap(),
            description: LessonMenuDescription::new("説明").unwrap(),
            duration: duration.map(|minutes| LessonDuration::new(minutes).unwrap()),
            is_published,
        }
    }

    fn live_menu() -> LessonMenu {
        LessonMenu::create(
            LessonMenuId::from_u128(7),
            COACH,
            &Coach::new([Sport::Tennis]).unwrap(),
            Sport::Tennis,
            LessonType::Live,
            content("フォーム改善", Some(60), true),
            at(1),
        )
        .unwrap()
    }

    fn use_case() -> UpdateLessonMenu<InMemoryLessonMenuRepository, FixedClock> {
        UpdateLessonMenu::new(
            InMemoryLessonMenuRepository::with_menus([live_menu()]),
            FixedClock(at(10)),
        )
    }

    fn command(account_id: AccountId, content: LessonMenuContent) -> UpdateLessonMenuCommand {
        UpdateLessonMenuCommand {
            account_id,
            lesson_menu_id: LessonMenuId::from_u128(7),
            content,
        }
    }

    #[tokio::test]
    async fn the_coach_revises_and_unpublishes_the_menu() {
        let use_case = use_case();

        let menu = use_case
            .execute(command(COACH, content("サーブ特訓", Some(90), false)))
            .await
            .unwrap();
        assert_eq!(menu.name().as_str(), "サーブ特訓");
        assert_eq!(menu.duration().map(LessonDuration::minutes), Some(90));
        assert!(!menu.is_published());
        assert_eq!((menu.created_at(), menu.updated_at()), (at(1), at(10)));
        assert_eq!(use_case.menus().all(), [menu]);
    }

    #[tokio::test]
    async fn another_account_cannot_revise_the_menu() {
        let use_case = use_case();

        for account in [OTHER_COACH, STUDENT] {
            let result = use_case
                .execute(command(account, content("乗っ取り", Some(30), false)))
                .await;
            assert!(
                matches!(
                    result,
                    Err(UpdateLessonMenuError::Invalid(
                        DomainError::NotTheLessonMenuCoach
                    ))
                ),
                "{result:?}"
            );
        }
        assert_eq!(use_case.menus().all(), [live_menu()], "何も変えない");
    }

    #[tokio::test]
    async fn invalid_or_unknown_menus_change_nothing() {
        let use_case = use_case();

        let result = use_case
            .execute(command(COACH, content("時間なし", None, true)))
            .await;
        assert!(
            matches!(
                result,
                Err(UpdateLessonMenuError::Invalid(
                    DomainError::LessonDurationRequired
                ))
            ),
            "{result:?}"
        );
        let result = use_case
            .execute(UpdateLessonMenuCommand {
                lesson_menu_id: LessonMenuId::from_u128(99),
                ..command(COACH, content("ない", Some(30), true))
            })
            .await;
        assert!(
            matches!(result, Err(UpdateLessonMenuError::LessonMenuNotFound)),
            "{result:?}"
        );
        assert_eq!(use_case.menus().all(), [live_menu()]);
    }
}
