use std::collections::BTreeSet;

use domain::{
    catalog::Sport,
    identity::AccountId,
    matching::{FavoriteCoach, MatchingError},
};

use super::FavoriteCoachRepository;
use crate::{Unavailable, clock::Clock, identity::AccountRepository};

/// お気に入りに入れる入力。`student_id` は認証したアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddFavoriteCoachCommand {
    pub student_id: AccountId,
    pub coach_id: AccountId,
    /// 呼んだクライアントが扱うスポーツ（student-app はゴルフ以外）。コーチがこのどれも提供しなければ、
    /// コーチがないものとして断る
    pub sports: BTreeSet<Sport>,
}

#[derive(Debug)]
pub enum AddFavoriteCoachError {
    /// `student_id` のアカウントがない
    StudentNotFound,
    /// `coach_id` のアカウントがないか、`sports` のどれも提供しないコーチ
    CoachNotFound,
    /// お気に入りの不変条件の違反（生徒でない・相手がコーチでない）
    Rejected(MatchingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<Unavailable> for AddFavoriteCoachError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

/// コーチをお気に入りに入れるユースケース（student-app の `addFavoriteCoach`。生徒が呼ぶ）。
///
/// 生徒とコーチのアカウントを読み、domain の [`FavoriteCoach::new`] で生徒がコーチを入れるかを確かめて保存する。
/// 同じコーチを何度入れても 1 件で、最初に入れた時刻のまま（保存先が組で守る）。行動イベントも境界間イベントも
/// 発火しない。
pub struct AddFavoriteCoach<F, R, C> {
    favorites: F,
    accounts: R,
    clock: C,
}

impl<F, R, C> AddFavoriteCoach<F, R, C>
where
    F: FavoriteCoachRepository,
    R: AccountRepository,
    C: Clock,
{
    pub fn new(favorites: F, accounts: R, clock: C) -> Self {
        Self {
            favorites,
            accounts,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: AddFavoriteCoachCommand,
    ) -> Result<(), AddFavoriteCoachError> {
        let student = self
            .accounts
            .find(command.student_id)
            .await?
            .ok_or(AddFavoriteCoachError::StudentNotFound)?;
        let coach = self
            .accounts
            .find(command.coach_id)
            .await?
            .ok_or(AddFavoriteCoachError::CoachNotFound)?;
        let favorite = FavoriteCoach::new(&student, &coach, self.clock.now())
            .map_err(AddFavoriteCoachError::Rejected)?;
        let offers_any = coach
            .as_coach()
            .is_some_and(|coach| command.sports.iter().any(|sport| coach.offers(*sport)));
        if !offers_any {
            return Err(AddFavoriteCoachError::CoachNotFound);
        }
        self.favorites.add(favorite).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        identity::{Account, Email, Role},
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::{
        FixedClock, InMemoryAccountRepository, InMemoryFavoriteCoachRepository, ManualClock,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(2);
    const GOLF_COACH: AccountId = AccountId::from_u128(3);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(4);

    fn account(id: AccountId, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            id,
            Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
            DisplayName::new("テスト").unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    async fn accounts() -> InMemoryAccountRepository {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(STUDENT, Role::Student, &[Sport::Tennis]),
            account(COACH, Role::Coach, &[Sport::Soccer, Sport::Golf]),
            account(GOLF_COACH, Role::Coach, &[Sport::Golf]),
            account(OTHER_STUDENT, Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        accounts
    }

    fn command(coach_id: AccountId) -> AddFavoriteCoachCommand {
        AddFavoriteCoachCommand {
            student_id: STUDENT,
            coach_id,
            sports: Sport::ALL
                .into_iter()
                .filter(|sport| *sport != Sport::Golf)
                .collect(),
        }
    }

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[tokio::test]
    async fn adds_a_coach_once_and_keeps_the_first_time() {
        let favorites = InMemoryFavoriteCoachRepository::default();
        let clock = ManualClock::new(at(10));
        let use_case = AddFavoriteCoach::new(favorites.clone(), accounts().await, clock.clone());

        use_case.execute(command(COACH)).await.unwrap();
        clock.set(at(20));
        use_case.execute(command(COACH)).await.unwrap();

        assert_eq!(
            favorites.find_by_student(STUDENT).await.unwrap(),
            [FavoriteCoach::restore(STUDENT, COACH, at(10))],
            "2 回入れても 1 件で、最初の時刻のまま"
        );
    }

    #[tokio::test]
    async fn unknown_or_out_of_scope_coach_is_coach_not_found() {
        let favorites = InMemoryFavoriteCoachRepository::default();
        let use_case =
            AddFavoriteCoach::new(favorites.clone(), accounts().await, FixedClock(at(10)));

        for coach_id in [AccountId::from_u128(99), GOLF_COACH] {
            let result = use_case.execute(command(coach_id)).await;
            assert!(
                matches!(result, Err(AddFavoriteCoachError::CoachNotFound)),
                "{coach_id:?}: {result:?}"
            );
        }
        assert!(favorites.find_by_student(STUDENT).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_non_coach_is_rejected_and_a_missing_student_is_student_not_found() {
        let favorites = InMemoryFavoriteCoachRepository::default();
        let use_case =
            AddFavoriteCoach::new(favorites.clone(), accounts().await, FixedClock(at(10)));

        let result = use_case.execute(command(OTHER_STUDENT)).await;
        assert!(
            matches!(
                result,
                Err(AddFavoriteCoachError::Rejected(MatchingError::NotACoach))
            ),
            "{result:?}"
        );
        let result = use_case
            .execute(AddFavoriteCoachCommand {
                student_id: COACH,
                ..command(COACH)
            })
            .await;
        assert!(
            matches!(
                result,
                Err(AddFavoriteCoachError::Rejected(MatchingError::NotAStudent))
            ),
            "{result:?}"
        );
        let result = use_case
            .execute(AddFavoriteCoachCommand {
                student_id: AccountId::from_u128(98),
                ..command(COACH)
            })
            .await;
        assert!(
            matches!(result, Err(AddFavoriteCoachError::StudentNotFound)),
            "{result:?}"
        );
        assert!(favorites.find_by_student(STUDENT).await.unwrap().is_empty());
    }
}
