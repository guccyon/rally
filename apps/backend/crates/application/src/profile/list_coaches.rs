use domain::identity::Account;

use crate::{Unavailable, identity::AccountRepository};

/// コーチの一覧のユースケース（admin-web の `listCoaches`。本部だけが呼ぶ）。
///
/// コーチのアカウントを表示名の順（Unicode のコードポイントの順。同じ表示名は ID の順）で返す。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListCoaches<R> {
    accounts: R,
}

impl<R> ListCoaches<R>
where
    R: AccountRepository,
{
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }

    pub async fn execute(&self) -> Result<Vec<Account>, Unavailable> {
        self.accounts.list_coaches().await
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        identity::{AccountId, Email, Role, Sport},
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

    #[tokio::test]
    async fn lists_only_coaches_in_display_name_order() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "あおき", Role::Student, &[Sport::Tennis]),
            account(3, "いとう", Role::Coach, &[Sport::Soccer, Sport::Golf]),
            account(4, "うえの", Role::Headquarters, &[]),
            // 同じ表示名は ID の順
            account(6, "いとう", Role::Coach, &[Sport::Baseball]),
            account(5, "いとう", Role::Coach, &[Sport::Basketball]),
        ] {
            accounts.insert(account).await.unwrap();
        }

        let coaches = ListCoaches::new(accounts).execute().await.unwrap();
        let listed: Vec<_> = coaches
            .iter()
            .map(|a| (a.id().as_u128(), a.display_name().as_str()))
            .collect();
        assert_eq!(
            listed,
            [(3, "いとう"), (5, "いとう"), (6, "いとう"), (1, "たなか")]
        );
    }

    #[tokio::test]
    async fn no_coaches_is_an_empty_list() {
        let accounts = InMemoryAccountRepository::default();
        accounts
            .insert(account(1, "あおき", Role::Student, &[Sport::Tennis]))
            .await
            .unwrap();

        let coaches = ListCoaches::new(accounts).execute().await.unwrap();
        assert!(coaches.is_empty());
    }
}
