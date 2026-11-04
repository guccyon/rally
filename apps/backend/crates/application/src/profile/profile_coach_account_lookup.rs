use std::collections::{BTreeMap, BTreeSet};

use domain::identity::{Account, AccountId};

use crate::{
    Unavailable, identity::AccountRepository, live_lesson::SessionCoachAccountLookup,
    scheduling::SlotCoachAccountLookup,
};

/// live-lesson の [`SessionCoachAccountLookup`]（本部が見る、終わっていないまま時間の過ぎた通話セッションの一覧を
/// 競技で絞る）と、scheduling の [`SlotCoachAccountLookup`]（本部が見る予約枠の需給と予約の一覧を競技で絞る）の profile による
/// 実装。アカウントの保存先（データは当面 identity の表）から読むだけで、何も変えない。
///
/// コーチでないアカウントと、ないアカウントは結果に含めない。
#[derive(Debug, Clone)]
pub struct ProfileCoachAccountLookup<R> {
    accounts: R,
}

impl<R> ProfileCoachAccountLookup<R> {
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }
}

impl<R> SessionCoachAccountLookup for ProfileCoachAccountLookup<R>
where
    R: AccountRepository,
{
    async fn find_coach_accounts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, Account>, Unavailable> {
        let mut coaches = BTreeMap::new();
        for &coach_id in coach_ids {
            let Some(account) = self.accounts.find(coach_id).await? else {
                continue;
            };
            if account.as_coach().is_some() {
                coaches.insert(coach_id, account);
            }
        }
        Ok(coaches)
    }
}

impl<R> SlotCoachAccountLookup for ProfileCoachAccountLookup<R>
where
    R: AccountRepository,
{
    async fn find_slot_coach_accounts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, Account>, Unavailable> {
        self.find_coach_accounts(coach_ids).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        catalog::Sport,
        identity::{Email, Role},
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::InMemoryAccountRepository;

    fn account(id: u128, role: Role) -> Account {
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new("テスト").unwrap(),
            role,
            [Sport::Tennis],
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn returns_coach_accounts_only() {
        let accounts = InMemoryAccountRepository::default();
        let coach = account(1, Role::Coach);
        for account in [coach.clone(), account(2, Role::Student)] {
            accounts.insert(account).await.unwrap();
        }
        let found = ProfileCoachAccountLookup::new(accounts)
            .find_coach_accounts(&BTreeSet::from([
                AccountId::from_u128(1),
                AccountId::from_u128(2),
                AccountId::from_u128(3),
            ]))
            .await
            .unwrap();
        assert_eq!(
            found,
            BTreeMap::from([(AccountId::from_u128(1), coach)]),
            "生徒・ないアカウントは含めない"
        );
    }

    #[tokio::test]
    async fn slot_coach_accounts_are_the_same_coach_accounts() {
        let accounts = InMemoryAccountRepository::default();
        let coach = account(1, Role::Coach);
        for account in [coach.clone(), account(2, Role::Student)] {
            accounts.insert(account).await.unwrap();
        }
        let found = ProfileCoachAccountLookup::new(accounts)
            .find_slot_coach_accounts(&BTreeSet::from([
                AccountId::from_u128(1),
                AccountId::from_u128(2),
                AccountId::from_u128(3),
            ]))
            .await
            .unwrap();
        assert_eq!(found, BTreeMap::from([(AccountId::from_u128(1), coach)]));
    }
}
