use domain::identity::{AccountId, Coach};

use crate::{Unavailable, catalog::CoachLookup, identity::AccountRepository};

/// catalog の [`CoachLookup`] の identity による実装。アカウントの保存先から読むだけで、何も変えない。
///
/// コーチでないアカウントと、ないアカウントは `None`。
#[derive(Debug, Clone)]
pub struct IdentityCoachLookup<R> {
    accounts: R,
}

impl<R> IdentityCoachLookup<R> {
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }
}

impl<R> CoachLookup for IdentityCoachLookup<R>
where
    R: AccountRepository,
{
    async fn find_coach(&self, coach_id: AccountId) -> Result<Option<Coach>, Unavailable> {
        Ok(self
            .accounts
            .find(coach_id)
            .await?
            .and_then(|account| account.as_coach().cloned()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        identity::{Account, Email, Role, Sport},
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::InMemoryAccountRepository;

    fn account(id: u128, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new("テスト").unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn finds_only_coaches() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, Role::Coach, &[Sport::Tennis, Sport::Golf]),
            account(2, Role::Student, &[Sport::Tennis]),
            account(3, Role::Headquarters, &[]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let lookup = IdentityCoachLookup::new(accounts);

        let coach = lookup
            .find_coach(AccountId::from_u128(1))
            .await
            .unwrap()
            .unwrap();
        assert!(coach.offers(Sport::Golf) && !coach.offers(Sport::Soccer));
        for id in [2, 3, 4] {
            assert_eq!(
                lookup.find_coach(AccountId::from_u128(id)).await.unwrap(),
                None,
                "{id}"
            );
        }
    }
}
