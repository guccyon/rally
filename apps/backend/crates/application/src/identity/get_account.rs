use domain::identity::{Account, AccountId};

use super::AccountRepository;
use crate::Unavailable;

/// アカウントの取得のユースケース（admin-web の `getMyAccount`。ログイン中の本人のアカウント）。照会だけで、何も保存しない。
///
/// どのアカウントを取るか（トークンが示す本人）と、見つからないときの応答は境界（interface）が決める。
pub struct GetAccount<R> {
    accounts: R,
}

impl<R> GetAccount<R>
where
    R: AccountRepository,
{
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }

    /// `account_id` のアカウント。なければ `None`。
    pub async fn execute(&self, account_id: AccountId) -> Result<Option<Account>, Unavailable> {
        self.accounts.find(account_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        identity::{Email, Role, Sport},
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::InMemoryAccountRepository;

    fn account(id: u128, display_name: &str, role: Role, sports: &[Sport]) -> Account {
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
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
            account(1, "本部", Role::Headquarters, &[]),
            account(2, "たなか", Role::Coach, &[Sport::Tennis, Sport::Golf]),
            account(3, "あおき", Role::Student, &[Sport::Soccer]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        accounts
    }

    #[tokio::test]
    async fn returns_the_account_of_each_role() {
        let use_case = GetAccount::new(accounts().await);
        for (id, role, display_name) in [
            (1, Role::Headquarters, "本部"),
            (2, Role::Coach, "たなか"),
            (3, Role::Student, "あおき"),
        ] {
            let found = use_case
                .execute(AccountId::from_u128(id))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(found.id(), AccountId::from_u128(id));
            assert_eq!(found.role(), role);
            assert_eq!(found.display_name().as_str(), display_name);
        }
    }

    #[tokio::test]
    async fn unknown_account_is_none() {
        let use_case = GetAccount::new(accounts().await);
        let found = use_case.execute(AccountId::from_u128(99)).await.unwrap();
        assert!(found.is_none());
    }
}
