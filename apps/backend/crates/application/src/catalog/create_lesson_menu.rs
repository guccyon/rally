use domain::{
    DomainError,
    catalog::{LessonMenu, LessonMenuContent},
    identity::{AccountId, Sport},
    lesson::LessonType,
};

use super::{CoachLookup, LessonMenuRepository};
use crate::{Unavailable, clock::Clock};

/// レッスンメニューの作成の入力。`coach_id` は認証したコーチのアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateLessonMenuCommand {
    pub coach_id: AccountId,
    pub sport: Sport,
    pub lesson_type: LessonType,
    pub content: LessonMenuContent,
}

#[derive(Debug)]
pub enum CreateLessonMenuError {
    /// `coach_id` のコーチのアカウントがない
    CoachNotFound,
    /// メニューがドメインの不変条件を満たさない（提供スポーツでない、1 回の時間がレッスンの種類に合わない）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// レッスンメニューの作成のユースケース（admin-web の `createLessonMenu`。コーチだけが呼ぶ）。
///
/// コーチ（提供スポーツ）を [`CoachLookup`] で読み、domain の [`LessonMenu::create`] でメニューを作って保存する。
/// 行動イベントも境界間イベントも発火しない。
pub struct CreateLessonMenu<M, K, C> {
    menus: M,
    coaches: K,
    clock: C,
}

impl<M, K, C> CreateLessonMenu<M, K, C>
where
    M: LessonMenuRepository,
    K: CoachLookup,
    C: Clock,
{
    pub fn new(menus: M, coaches: K, clock: C) -> Self {
        Self {
            menus,
            coaches,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CreateLessonMenuCommand,
    ) -> Result<LessonMenu, CreateLessonMenuError> {
        let coach = self
            .coaches
            .find_coach(command.coach_id)
            .await
            .map_err(CreateLessonMenuError::Unavailable)?
            .ok_or(CreateLessonMenuError::CoachNotFound)?;
        let menu = LessonMenu::create(
            self.menus.next_id(),
            command.coach_id,
            &coach,
            command.sport,
            command.lesson_type,
            command.content,
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
        catalog::{LessonDuration, LessonMenuDescription, LessonMenuName},
        identity::{Account, Email, Role},
        profile::DisplayName,
    };

    use super::*;
    use crate::{
        identity::{AccountRepository, IdentityCoachLookup},
        in_memory::{FixedClock, InMemoryAccountRepository, InMemoryLessonMenuRepository},
    };

    type UseCase = CreateLessonMenu<
        InMemoryLessonMenuRepository,
        IdentityCoachLookup<InMemoryAccountRepository>,
        FixedClock,
    >;

    const COACH: AccountId = AccountId::from_u128(1);
    const STUDENT: AccountId = AccountId::from_u128(2);
    const HEADQUARTERS: AccountId = AccountId::from_u128(3);

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

    async fn use_case() -> UseCase {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(COACH, Role::Coach, &[Sport::Tennis, Sport::Golf]),
            account(STUDENT, Role::Student, &[Sport::Tennis]),
            account(HEADQUARTERS, Role::Headquarters, &[]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        CreateLessonMenu::new(
            InMemoryLessonMenuRepository::default(),
            IdentityCoachLookup::new(accounts),
            FixedClock(at(10)),
        )
    }

    fn command(
        coach_id: AccountId,
        sport: Sport,
        lesson_type: LessonType,
        duration: Option<i64>,
    ) -> CreateLessonMenuCommand {
        CreateLessonMenuCommand {
            coach_id,
            sport,
            lesson_type,
            content: LessonMenuContent {
                name: LessonMenuName::new("フォーム改善").unwrap(),
                description: LessonMenuDescription::new("初心者向け").unwrap(),
                duration: duration.map(|minutes| LessonDuration::new(minutes).unwrap()),
                is_published: true,
            },
        }
    }

    #[tokio::test]
    async fn the_coach_creates_a_menu_for_a_sport_they_offer() {
        let use_case = use_case().await;

        let menu = use_case
            .execute(command(COACH, Sport::Tennis, LessonType::Live, Some(60)))
            .await
            .unwrap();
        assert_eq!(menu.coach_id(), COACH);
        assert_eq!(menu.sport(), Sport::Tennis);
        assert_eq!(menu.lesson_type(), LessonType::Live);
        assert_eq!(menu.duration().map(LessonDuration::minutes), Some(60));
        assert_eq!(menu.created_at(), at(10));
        assert_eq!(use_case.menus().all(), [menu]);

        let golf = use_case
            .execute(command(COACH, Sport::Golf, LessonType::Async, None))
            .await
            .unwrap();
        assert_eq!(golf.duration(), None);
        assert_eq!(use_case.menus().all().len(), 2);
    }

    #[tokio::test]
    async fn invalid_menus_are_not_saved() {
        let use_case = use_case().await;

        for (command, expected) in [
            (
                command(COACH, Sport::Soccer, LessonType::Async, None),
                DomainError::SportNotOfferedByCoach {
                    sport: Sport::Soccer,
                },
            ),
            (
                command(COACH, Sport::Tennis, LessonType::Live, None),
                DomainError::LessonDurationRequired,
            ),
            (
                command(COACH, Sport::Tennis, LessonType::Async, Some(30)),
                DomainError::LessonDurationNotApplicable,
            ),
        ] {
            let result = use_case.execute(command).await;
            assert!(
                matches!(&result, Err(CreateLessonMenuError::Invalid(error)) if *error == expected),
                "{result:?}"
            );
        }
        assert!(use_case.menus().all().is_empty());
    }

    #[tokio::test]
    async fn a_student_or_headquarters_or_unknown_account_is_not_a_coach() {
        let use_case = use_case().await;

        for id in [STUDENT, HEADQUARTERS, AccountId::from_u128(99)] {
            let result = use_case
                .execute(command(id, Sport::Tennis, LessonType::Async, None))
                .await;
            assert!(
                matches!(result, Err(CreateLessonMenuError::CoachNotFound)),
                "{id:?}: {result:?}"
            );
        }
        assert!(use_case.menus().all().is_empty());
    }
}
