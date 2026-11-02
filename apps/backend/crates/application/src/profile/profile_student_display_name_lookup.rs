use std::collections::{BTreeMap, BTreeSet};

use domain::{identity::AccountId, profile::DisplayName};

use crate::{
    Unavailable, async_lesson::VideoStudentDisplayNameLookup, identity::AccountRepository,
    live_lesson::SessionStudentDisplayNameLookup, review::ReviewerDisplayNameLookup,
    scheduling::StudentDisplayNameLookup,
};

/// scheduling の [`StudentDisplayNameLookup`] と async-lesson の [`VideoStudentDisplayNameLookup`] と review の
/// [`ReviewerDisplayNameLookup`] と live-lesson の [`SessionStudentDisplayNameLookup`] の profile による実装。アカウントの保存先（データは当面 identity の表）から読むだけで、何も変えない。
///
/// 生徒でないアカウントと、ないアカウントは結果に含めない。
#[derive(Debug, Clone)]
pub struct ProfileStudentDisplayNameLookup<R> {
    accounts: R,
}

impl<R> ProfileStudentDisplayNameLookup<R> {
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }
}

impl<R> ProfileStudentDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn student_display_names(
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

impl<R> StudentDisplayNameLookup for ProfileStudentDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn find_student_display_names(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        self.student_display_names(student_ids).await
    }
}

impl<R> VideoStudentDisplayNameLookup for ProfileStudentDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn find_student_display_names(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        self.student_display_names(student_ids).await
    }
}

impl<R> ReviewerDisplayNameLookup for ProfileStudentDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn find_student_display_names(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        self.student_display_names(student_ids).await
    }
}

impl<R> SessionStudentDisplayNameLookup for ProfileStudentDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn find_student_display_names(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        self.student_display_names(student_ids).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
    };

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
        let lookup = ProfileStudentDisplayNameLookup::new(accounts);
        let expected = BTreeMap::from([
            (AccountId::from_u128(1), DisplayName::new("あおき").unwrap()),
            (AccountId::from_u128(3), DisplayName::new("いとう").unwrap()),
        ]);

        assert_eq!(
            StudentDisplayNameLookup::find_student_display_names(&lookup, &ids)
                .await
                .unwrap(),
            expected,
            "コーチ（2）とないアカウント（4）は含めない"
        );
        assert_eq!(
            VideoStudentDisplayNameLookup::find_student_display_names(&lookup, &ids)
                .await
                .unwrap(),
            expected,
            "async-lesson のポートも同じ答え"
        );
        assert_eq!(
            ReviewerDisplayNameLookup::find_student_display_names(&lookup, &ids)
                .await
                .unwrap(),
            expected,
            "review のポートも同じ答え"
        );
        assert_eq!(
            SessionStudentDisplayNameLookup::find_student_display_names(&lookup, &ids)
                .await
                .unwrap(),
            expected,
            "live-lesson のポートも同じ答え"
        );
    }

    #[tokio::test]
    async fn no_ids_is_an_empty_map() {
        let lookup = ProfileStudentDisplayNameLookup::new(InMemoryAccountRepository::default());
        let found = StudentDisplayNameLookup::find_student_display_names(&lookup, &BTreeSet::new())
            .await
            .unwrap();
        assert!(found.is_empty());
    }
}
