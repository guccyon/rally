use domain::{
    DomainError,
    catalog::{LessonMenu, LessonMenuContent, LessonMenuId},
    identity::AccountId,
};

use super::LessonMenuRepository;
use crate::{Unavailable, identity::AccountRepository};

/// レッスンメニューの変更の入力。`coach_id` は認証したコーチのアカウント（役割の確認は境界が行う。メニューの
/// コーチかは domain が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateLessonMenuCommand {
    pub coach_id: AccountId,
    pub lesson_menu_id: LessonMenuId,
    pub content: LessonMenuContent,
    pub is_published: bool,
}

#[derive(Debug)]
pub enum UpdateLessonMenuError {
    /// メニューがない
    LessonMenuNotFound,
    /// 変更がドメインの不変条件を満たさない（メニューのコーチでない、スポーツがコーチの提供スポーツでない）
    Invalid(DomainError),
    /// `coach_id` のコーチのアカウントがない（ID のアカウントがないか、コーチでない）
    CoachNotFound,
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// レッスンメニューの変更のユースケース（admin-web の `updateMyLessonMenu`。コーチだけが呼ぶ）。中身と公開中かを
/// 丸ごと置き換え（非公開にするのもこれ）、変えたあとのメニューを返す。
///
/// メニューとコーチのアカウントを読み、domain の [`LessonMenu::revise`] で確かめて保存する。拒否では何も保存しない。
/// 行動イベントも境界間イベントも発火しない。
pub struct UpdateLessonMenu<M, R> {
    menus: M,
    accounts: R,
}

impl<M, R> UpdateLessonMenu<M, R>
where
    M: LessonMenuRepository,
    R: AccountRepository,
{
    pub fn new(menus: M, accounts: R) -> Self {
        Self { menus, accounts }
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
        let account = self
            .accounts
            .find(command.coach_id)
            .await
            .map_err(UpdateLessonMenuError::Unavailable)?;
        let coach = account
            .as_ref()
            .and_then(|account| account.as_coach())
            .ok_or(UpdateLessonMenuError::CoachNotFound)?;
        menu.revise(
            command.coach_id,
            coach,
            command.content,
            command.is_published,
        )
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
        catalog::{LessonFormat, LessonMenuDescription, LessonMenuName},
        identity::{Account, Email, Role, Sport},
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::{InMemoryAccountRepository, InMemoryLessonMenuRepository};

    const COACH: AccountId = AccountId::from_u128(2);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);
    const STUDENT: AccountId = AccountId::from_u128(3);
    const MENU: LessonMenuId = LessonMenuId::from_u128(10);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn account(id: AccountId, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            id,
            Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
            DisplayName::new("テスト").unwrap(),
            role,
            sports.iter().copied(),
            at(0),
        )
        .unwrap()
    }

    fn content(sport: Sport, name: &str) -> LessonMenuContent {
        LessonMenuContent {
            sport,
            format: LessonFormat::Async,
            name: LessonMenuName::new(name).unwrap(),
            description: LessonMenuDescription::default(),
        }
    }

    fn menu() -> LessonMenu {
        LessonMenu::restore(MENU, COACH, content(Sport::Tennis, "前の名前"), true, at(5))
    }

    async fn use_case() -> UpdateLessonMenu<InMemoryLessonMenuRepository, InMemoryAccountRepository>
    {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(COACH, Role::Coach, &[Sport::Tennis, Sport::Soccer]),
            account(OTHER_COACH, Role::Coach, &[Sport::Tennis]),
            account(STUDENT, Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        UpdateLessonMenu::new(InMemoryLessonMenuRepository::with_menus([menu()]), accounts)
    }

    fn command(coach_id: AccountId, sport: Sport, is_published: bool) -> UpdateLessonMenuCommand {
        UpdateLessonMenuCommand {
            coach_id,
            lesson_menu_id: MENU,
            content: content(sport, "新しい名前"),
            is_published,
        }
    }

    #[tokio::test]
    async fn the_coach_revises_and_unpublishes_its_menu() {
        let use_case = use_case().await;
        let updated = use_case
            .execute(command(COACH, Sport::Soccer, false))
            .await
            .unwrap();
        assert_eq!(updated.id(), MENU);
        assert_eq!(updated.sport(), Sport::Soccer);
        assert_eq!(updated.name().as_str(), "新しい名前");
        assert!(!updated.is_published());
        assert_eq!(updated.created_at(), at(5));
        assert_eq!(use_case.menus().all(), vec![updated]);
    }

    #[tokio::test]
    async fn another_coach_cannot_revise_the_menu() {
        let use_case = use_case().await;
        let result = use_case
            .execute(command(OTHER_COACH, Sport::Tennis, false))
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
        assert_eq!(use_case.menus().all(), vec![menu()], "何も変えない");
    }

    #[tokio::test]
    async fn rejected_updates_change_nothing() {
        let use_case = use_case().await;
        let result = use_case.execute(command(COACH, Sport::Golf, true)).await;
        assert!(
            matches!(
                result,
                Err(UpdateLessonMenuError::Invalid(
                    DomainError::SportNotOfferedByCoach { sport: Sport::Golf }
                ))
            ),
            "{result:?}"
        );
        let result = use_case
            .execute(command(STUDENT, Sport::Tennis, true))
            .await;
        assert!(
            matches!(result, Err(UpdateLessonMenuError::CoachNotFound)),
            "{result:?}"
        );
        let result = use_case
            .execute(UpdateLessonMenuCommand {
                lesson_menu_id: LessonMenuId::from_u128(99),
                ..command(COACH, Sport::Tennis, true)
            })
            .await;
        assert!(
            matches!(result, Err(UpdateLessonMenuError::LessonMenuNotFound)),
            "{result:?}"
        );
        assert_eq!(use_case.menus().all(), vec![menu()]);
    }
}
