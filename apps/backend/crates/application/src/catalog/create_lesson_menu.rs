use domain::{
    DomainError,
    catalog::{LessonMenu, LessonMenuContent},
    identity::AccountId,
};

use super::LessonMenuRepository;
use crate::{Unavailable, clock::Clock, identity::AccountRepository};

/// レッスンメニューの作成の入力。`coach_id` は認証したコーチのアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateLessonMenuCommand {
    pub coach_id: AccountId,
    pub content: LessonMenuContent,
    pub is_published: bool,
}

#[derive(Debug)]
pub enum CreateLessonMenuError {
    /// メニューがドメインの不変条件を満たさない（スポーツがコーチの提供スポーツでない）
    Invalid(DomainError),
    /// `coach_id` のコーチのアカウントがない（ID のアカウントがないか、コーチでない）
    CoachNotFound,
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// レッスンメニューの作成のユースケース（admin-web の `createMyLessonMenu`。コーチだけが呼ぶ）。
///
/// コーチのアカウントを読み、domain の [`LessonMenu::create`] で提供スポーツのメニューかを確かめて保存する。
/// 行動イベントも境界間イベントも発火しない。
pub struct CreateLessonMenu<M, R, C> {
    menus: M,
    accounts: R,
    clock: C,
}

impl<M, R, C> CreateLessonMenu<M, R, C>
where
    M: LessonMenuRepository,
    R: AccountRepository,
    C: Clock,
{
    pub fn new(menus: M, accounts: R, clock: C) -> Self {
        Self {
            menus,
            accounts,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CreateLessonMenuCommand,
    ) -> Result<LessonMenu, CreateLessonMenuError> {
        let account = self
            .accounts
            .find(command.coach_id)
            .await
            .map_err(CreateLessonMenuError::Unavailable)?;
        let coach = account
            .as_ref()
            .and_then(|account| account.as_coach())
            .ok_or(CreateLessonMenuError::CoachNotFound)?;
        let menu = LessonMenu::create(
            self.menus.next_id(),
            command.coach_id,
            coach,
            command.content,
            command.is_published,
            self.clock.now(),
        )
        .map_err(CreateLessonMenuError::Invalid)?;
        self.menus
            .insert(menu.clone())
            .await
            .map_err(CreateLessonMenuError::Unavailable)?;
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
        catalog::Sport,
        catalog::{LessonDuration, LessonFormat, LessonMenuDescription, LessonMenuName},
        identity::{Account, Email, Role},
        lesson::LessonType,
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryAccountRepository, InMemoryLessonMenuRepository};

    const HEADQUARTERS: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(2);
    const STUDENT: AccountId = AccountId::from_u128(3);

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn account(id: AccountId, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            id,
            Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
            DisplayName::new("テスト").unwrap(),
            role,
            sports.iter().copied(),
            now(),
        )
        .unwrap()
    }

    async fn use_case()
    -> CreateLessonMenu<InMemoryLessonMenuRepository, InMemoryAccountRepository, FixedClock> {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(HEADQUARTERS, Role::Headquarters, &[]),
            account(COACH, Role::Coach, &[Sport::Tennis, Sport::Golf]),
            account(STUDENT, Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        CreateLessonMenu::new(
            InMemoryLessonMenuRepository::default(),
            accounts,
            FixedClock(now()),
        )
    }

    fn command(coach_id: AccountId, sport: Sport) -> CreateLessonMenuCommand {
        CreateLessonMenuCommand {
            coach_id,
            content: LessonMenuContent {
                sport,
                format: LessonFormat::new(
                    LessonType::Live,
                    Some(LessonDuration::from_minutes(60).unwrap()),
                )
                .unwrap(),
                name: LessonMenuName::new("フォア改善").unwrap(),
                description: LessonMenuDescription::new("初心者向け").unwrap(),
            },
            is_published: true,
        }
    }

    #[tokio::test]
    async fn the_coach_creates_a_menu_and_it_is_saved() {
        let use_case = use_case().await;
        let menu = use_case.execute(command(COACH, Sport::Golf)).await.unwrap();
        assert_eq!(menu.coach_id(), COACH);
        assert_eq!(menu.sport(), Sport::Golf);
        assert_eq!(menu.created_at(), now());
        assert!(menu.is_published());
        assert_eq!(use_case.menus().all(), vec![menu]);
    }

    #[tokio::test]
    async fn a_sport_the_coach_does_not_offer_is_invalid_and_nothing_is_saved() {
        let use_case = use_case().await;
        let result = use_case.execute(command(COACH, Sport::Soccer)).await;
        assert!(
            matches!(
                result,
                Err(CreateLessonMenuError::Invalid(
                    DomainError::SportNotOfferedByCoach {
                        sport: Sport::Soccer
                    }
                ))
            ),
            "{result:?}"
        );
        assert!(use_case.menus().all().is_empty());
    }

    #[tokio::test]
    async fn non_coach_or_unknown_account_is_coach_not_found() {
        let use_case = use_case().await;
        for id in [HEADQUARTERS, STUDENT, AccountId::from_u128(99)] {
            let result = use_case.execute(command(id, Sport::Tennis)).await;
            assert!(
                matches!(result, Err(CreateLessonMenuError::CoachNotFound)),
                "{id:?}: {result:?}"
            );
        }
        assert!(use_case.menus().all().is_empty());
    }
}
