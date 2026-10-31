use domain::catalog::Sport;

use crate::{
    Unavailable,
    identity::AccountRepository,
    matching::{CoachCandidate, CoachCandidateLookup},
};

/// matching の [`CoachCandidateLookup`] の profile による実装。アカウントの保存先（データは当面 identity の表）から
/// 読むだけで、何も変えない。
#[derive(Debug, Clone)]
pub struct ProfileCoachCandidateLookup<R> {
    accounts: R,
}

impl<R> ProfileCoachCandidateLookup<R> {
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }
}

impl<R> CoachCandidateLookup for ProfileCoachCandidateLookup<R>
where
    R: AccountRepository,
{
    async fn find_coaches_offering(
        &self,
        sport: Sport,
    ) -> Result<Vec<CoachCandidate>, Unavailable> {
        Ok(self
            .accounts
            .list_coaches()
            .await?
            .into_iter()
            .filter(|account| account.as_coach().is_some_and(|coach| coach.offers(sport)))
            .map(|account| CoachCandidate {
                coach_id: account.id(),
                display_name: account.display_name().clone(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        identity::{Account, AccountId, Email, Role},
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::InMemoryAccountRepository;

    fn account(id: u128, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new(format!("ユーザー{id}")).unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn returns_only_coaches_offering_the_sport() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, Role::Coach, &[Sport::Tennis]),
            account(2, Role::Student, &[Sport::Tennis]),
            account(3, Role::Coach, &[Sport::Soccer, Sport::Tennis]),
            account(4, Role::Coach, &[Sport::Golf]),
        ] {
            accounts.insert(account).await.unwrap();
        }

        let lookup = ProfileCoachCandidateLookup::new(accounts);
        let mut ids: Vec<u128> = lookup
            .find_coaches_offering(Sport::Tennis)
            .await
            .unwrap()
            .iter()
            .map(|c| c.coach_id.as_u128())
            .collect();
        ids.sort();
        assert_eq!(ids, [1, 3], "生徒（2）とゴルフだけのコーチ（4）は含めない");
        let golf = lookup.find_coaches_offering(Sport::Golf).await.unwrap();
        assert_eq!(
            golf,
            [CoachCandidate {
                coach_id: AccountId::from_u128(4),
                display_name: DisplayName::new("ユーザー4").unwrap(),
            }]
        );
    }
}
