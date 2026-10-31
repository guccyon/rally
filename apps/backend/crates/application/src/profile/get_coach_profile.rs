use domain::identity::AccountId;

use crate::{Unavailable, identity::AccountRepository, profile::CoachProfile};

/// コーチの紹介の照会のユースケース（student-app の `getCoachProfile`）。アカウントの保存先（データは当面 identity の表）
/// から読むだけで、何も変えない。
///
/// `coach_id` がコーチのアカウントでなければ（生徒・本部のアカウントや、ないアカウント）`None`。誰が呼べるか（生徒だけ）と、
/// どのスポーツを見せるかは境界（interface）が決める。
pub struct GetCoachProfile<R> {
    accounts: R,
}

impl<R> GetCoachProfile<R>
where
    R: AccountRepository,
{
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }

    pub async fn execute(&self, coach_id: AccountId) -> Result<Option<CoachProfile>, Unavailable> {
        let Some(account) = self.accounts.find(coach_id).await? else {
            return Ok(None);
        };
        Ok(account.as_coach().map(|coach| CoachProfile {
            display_name: account.display_name().clone(),
            bio: coach.bio().clone(),
            sports: coach.sports().clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, time::SystemTime};

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
        profile::{CoachBio, DisplayName},
    };

    use super::*;
    use crate::in_memory::InMemoryAccountRepository;

    const COACH: AccountId = AccountId::from_u128(1);
    const QUIET_COACH: AccountId = AccountId::from_u128(2);
    const STUDENT: AccountId = AccountId::from_u128(3);
    const HEADQUARTERS: AccountId = AccountId::from_u128(4);

    fn account(id: AccountId, display_name: &str, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            id,
            Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
            DisplayName::new(display_name).unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    async fn accounts() -> InMemoryAccountRepository {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(
                COACH,
                "たなか",
                Role::Coach,
                &[Sport::Soccer, Sport::Tennis],
            ),
            account(QUIET_COACH, "いとう", Role::Coach, &[Sport::Golf]),
            account(STUDENT, "あおき", Role::Student, &[Sport::Tennis]),
            account(HEADQUARTERS, "うえの", Role::Headquarters, &[]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        accounts
            .update_coach_bio(COACH, CoachBio::new("テニス歴 20 年").unwrap())
            .await
            .unwrap();
        accounts
    }

    #[tokio::test]
    async fn returns_the_display_name_bio_and_sports_of_a_coach() {
        let use_case = GetCoachProfile::new(accounts().await);

        assert_eq!(
            use_case.execute(COACH).await.unwrap(),
            Some(CoachProfile {
                display_name: DisplayName::new("たなか").unwrap(),
                bio: CoachBio::new("テニス歴 20 年").unwrap(),
                sports: BTreeSet::from([Sport::Tennis, Sport::Soccer]),
            })
        );
        assert_eq!(
            use_case.execute(QUIET_COACH).await.unwrap(),
            Some(CoachProfile {
                display_name: DisplayName::new("いとう").unwrap(),
                bio: CoachBio::default(),
                sports: BTreeSet::from([Sport::Golf]),
            }),
            "自己紹介文を書いていなければ空。ゴルフも含めて返す（見せるかは境界が決める）"
        );
    }

    #[tokio::test]
    async fn non_coach_or_unknown_account_is_none() {
        let use_case = GetCoachProfile::new(accounts().await);

        for id in [STUDENT, HEADQUARTERS, AccountId::from_u128(99)] {
            assert_eq!(use_case.execute(id).await.unwrap(), None, "{id:?}");
        }
    }
}
