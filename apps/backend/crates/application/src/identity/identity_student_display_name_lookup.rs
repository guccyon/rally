use std::collections::{BTreeMap, BTreeSet};

use domain::identity::{AccountId, DisplayName};

use super::AccountRepository;
use crate::{Unavailable, scheduling::StudentDisplayNameLookup};

/// scheduling の [`StudentDisplayNameLookup`] の identity による実装。アカウントの保存先から読むだけで、何も変えない。
///
/// 生徒でないアカウントと、ないアカウントは結果に含めない。
#[derive(Debug, Clone)]
pub struct IdentityStudentDisplayNameLookup<R> {
    accounts: R,
}

impl<R> IdentityStudentDisplayNameLookup<R> {
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }
}

impl<R> StudentDisplayNameLookup for IdentityStudentDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn find_student_display_names(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        let mut names = BTreeMap::new();
        for &student_id in student_ids {
            let Some(account) = self.accounts.find(student_id).await? else {
                continue;
            };
            if account.as_student().is_none() {
                continue;
            }
            names.insert(student_id, account.display_name().clone());
        }
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::identity::{Account, Email, Role, Sport};

    use super::*;
    use crate::in_memory::InMemoryAccountRepository;

    fn account(id: u128, display_name: &str, role: Role) -> Account {
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new(display_name).unwrap(),
            role,
            [Sport::Tennis],
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn returns_display_names_of_students_only() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "あおき", Role::Student),
            account(2, "たなか", Role::Coach),
            account(3, "いとう", Role::Student),
        ] {
            accounts.insert(account).await.unwrap();
        }

        let ids = [1, 2, 3, 4].map(AccountId::from_u128).into();
        let found = IdentityStudentDisplayNameLookup::new(accounts)
            .find_student_display_names(&ids)
            .await
            .unwrap();

        assert_eq!(
            found,
            BTreeMap::from([
                (AccountId::from_u128(1), DisplayName::new("あおき").unwrap()),
                (AccountId::from_u128(3), DisplayName::new("いとう").unwrap()),
            ]),
            "コーチ（2）とないアカウント（4）は含めない"
        );
    }

    #[tokio::test]
    async fn no_ids_is_an_empty_map() {
        let found = IdentityStudentDisplayNameLookup::new(InMemoryAccountRepository::default())
            .find_student_display_names(&BTreeSet::new())
            .await
            .unwrap();
        assert!(found.is_empty());
    }
}
