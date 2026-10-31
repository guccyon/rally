use std::collections::{BTreeMap, BTreeSet};

use domain::{identity::AccountId, profile::DisplayName};

use crate::{
    Unavailable, async_lesson::VideoCoachDisplayNameLookup,
    catalog::LessonMenuCoachDisplayNameLookup, identity::AccountRepository,
    lesson::LessonCompletionCoachDisplayNameLookup, review::ReviewedCoachDisplayNameLookup,
    scheduling::BookingCoachDisplayNameLookup,
};

/// async-lesson の [`VideoCoachDisplayNameLookup`] と catalog の [`LessonMenuCoachDisplayNameLookup`] と完了記録の
/// [`LessonCompletionCoachDisplayNameLookup`] と scheduling の [`BookingCoachDisplayNameLookup`]（本部が見る予約の一覧）と
/// review の [`ReviewedCoachDisplayNameLookup`]（本部が見る低い評価の一覧）の profile による実装。アカウントの保存先
/// （データは当面 identity の表）から読むだけで、何も変えない。
///
/// コーチでないアカウントと、ないアカウントは結果に含めない。
#[derive(Debug, Clone)]
pub struct ProfileCoachDisplayNameLookup<R> {
    accounts: R,
}

impl<R> ProfileCoachDisplayNameLookup<R> {
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }
}

impl<R> ProfileCoachDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        let mut names = BTreeMap::new();
        for &coach_id in coach_ids {
            let Some(account) = self.accounts.find(coach_id).await? else {
                continue;
            };
            if account.as_coach().is_none() {
                continue;
            }
            names.insert(coach_id, account.display_name().clone());
        }
        Ok(names)
    }
}

impl<R> VideoCoachDisplayNameLookup for ProfileCoachDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn find_coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        self.coach_display_names(coach_ids).await
    }
}

impl<R> LessonMenuCoachDisplayNameLookup for ProfileCoachDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn find_coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        self.coach_display_names(coach_ids).await
    }
}

impl<R> LessonCompletionCoachDisplayNameLookup for ProfileCoachDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn find_coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        self.coach_display_names(coach_ids).await
    }
}

impl<R> BookingCoachDisplayNameLookup for ProfileCoachDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn find_coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        self.coach_display_names(coach_ids).await
    }
}

impl<R> ReviewedCoachDisplayNameLookup for ProfileCoachDisplayNameLookup<R>
where
    R: AccountRepository,
{
    async fn find_coach_display_names(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
        self.coach_display_names(coach_ids).await
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
    async fn returns_display_names_of_coaches_only() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach),
            account(2, "あおき", Role::Student),
            account(3, "いとう", Role::Coach),
        ] {
            accounts.insert(account).await.unwrap();
        }

        let ids = [1, 2, 3, 4].map(AccountId::from_u128).into();
        let lookup = ProfileCoachDisplayNameLookup::new(accounts);
        let found = VideoCoachDisplayNameLookup::find_coach_display_names(&lookup, &ids)
            .await
            .unwrap();
        assert_eq!(
            LessonMenuCoachDisplayNameLookup::find_coach_display_names(&lookup, &ids)
                .await
                .unwrap(),
            found,
            "catalog にも同じ表示名を渡す"
        );
        assert_eq!(
            LessonCompletionCoachDisplayNameLookup::find_coach_display_names(&lookup, &ids)
                .await
                .unwrap(),
            found,
            "完了記録にも同じ表示名を渡す"
        );
        assert_eq!(
            BookingCoachDisplayNameLookup::find_coach_display_names(&lookup, &ids)
                .await
                .unwrap(),
            found,
            "予約の一覧にも同じ表示名を渡す"
        );
        assert_eq!(
            ReviewedCoachDisplayNameLookup::find_coach_display_names(&lookup, &ids)
                .await
                .unwrap(),
            found,
            "review にも同じ表示名を渡す"
        );
        assert_eq!(
            found,
            BTreeMap::from([
                (AccountId::from_u128(1), DisplayName::new("たなか").unwrap()),
                (AccountId::from_u128(3), DisplayName::new("いとう").unwrap()),
            ]),
            "生徒（2）とないアカウント（4）は含めない"
        );
    }
}
