use domain::{identity::AccountId, profile::CoachBio};

use crate::{Unavailable, profile::CoachProfileRepository};

/// コーチの自己紹介文の照会のユースケース（admin-web の `getMyAccount` に添える自己紹介文）。照会だけで、何も保存しない。
///
/// どのアカウントのものを取るか（トークンが示す本人）と、コーチでないときにどう見せるかは境界（interface）が決める。
pub struct GetCoachBio<P> {
    profiles: P,
}

impl<P> GetCoachBio<P>
where
    P: CoachProfileRepository,
{
    pub fn new(profiles: P) -> Self {
        Self { profiles }
    }

    /// コーチ `coach_id` の自己紹介文（書いていなければ空）。コーチのアカウントでなければ `None`。
    pub async fn execute(&self, coach_id: AccountId) -> Result<Option<CoachBio>, Unavailable> {
        self.profiles.find_coach_bio(coach_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
        profile::DisplayName,
    };

    use super::*;
    use crate::{identity::AccountRepository, in_memory::InMemoryAccountRepository};

    const COACH: AccountId = AccountId::from_u128(1);
    const QUIET_COACH: AccountId = AccountId::from_u128(2);
    const STUDENT: AccountId = AccountId::from_u128(3);
    const HEADQUARTERS: AccountId = AccountId::from_u128(4);

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

    async fn use_case() -> GetCoachBio<InMemoryAccountRepository> {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(COACH, Role::Coach, &[Sport::Tennis]),
            account(QUIET_COACH, Role::Coach, &[Sport::Golf]),
            account(STUDENT, Role::Student, &[Sport::Tennis]),
            account(HEADQUARTERS, Role::Headquarters, &[]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        accounts
            .update_coach_bio(COACH, CoachBio::new("テニス歴 20 年").unwrap())
            .await
            .unwrap();
        GetCoachBio::new(accounts)
    }

    #[tokio::test]
    async fn returns_the_bio_of_a_coach_or_empty_if_not_written() {
        let use_case = use_case().await;

        assert_eq!(
            use_case.execute(COACH).await.unwrap(),
            Some(CoachBio::new("テニス歴 20 年").unwrap())
        );
        assert_eq!(
            use_case.execute(QUIET_COACH).await.unwrap(),
            Some(CoachBio::default())
        );
    }

    #[tokio::test]
    async fn non_coach_or_unknown_account_is_none() {
        let use_case = use_case().await;

        for id in [STUDENT, HEADQUARTERS, AccountId::from_u128(99)] {
            assert_eq!(use_case.execute(id).await.unwrap(), None, "{id:?}");
        }
    }
}
