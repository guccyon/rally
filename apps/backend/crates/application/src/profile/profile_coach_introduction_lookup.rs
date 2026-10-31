use std::collections::{BTreeMap, BTreeSet};

use domain::identity::AccountId;

use crate::{
    Unavailable,
    identity::AccountRepository,
    scheduling::{CoachIntroduction, CoachIntroductionLookup},
};

/// scheduling の [`CoachIntroductionLookup`] の profile による実装。アカウントの保存先（データは当面 identity の表）から
/// 読むだけで、何も変えない。
///
/// コーチでないアカウントと、ないアカウントは結果に含めない。
#[derive(Debug, Clone)]
pub struct ProfileCoachIntroductionLookup<R> {
    accounts: R,
}

impl<R> ProfileCoachIntroductionLookup<R> {
    pub fn new(accounts: R) -> Self {
        Self { accounts }
    }
}

impl<R> CoachIntroductionLookup for ProfileCoachIntroductionLookup<R>
where
    R: AccountRepository,
{
    async fn find_coach_introductions(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, CoachIntroduction>, Unavailable> {
        let mut introductions = BTreeMap::new();
        for &coach_id in coach_ids {
            let Some(account) = self.accounts.find(coach_id).await? else {
                continue;
            };
            let Some(coach) = account.as_coach() else {
                continue;
            };
            introductions.insert(
                coach_id,
                CoachIntroduction {
                    display_name: account.display_name().clone(),
                    bio: coach.bio().clone(),
                },
            );
        }
        Ok(introductions)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
        profile::{CoachBio, DisplayName},
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
    async fn returns_display_names_and_bios_of_coaches_only() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "あおき", Role::Student, &[Sport::Tennis]),
            account(3, "いとう", Role::Coach, &[Sport::Soccer]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let bio = CoachBio::new("テニス歴 20 年").unwrap();
        accounts
            .update_coach_bio(AccountId::from_u128(1), bio.clone())
            .await
            .unwrap();

        let ids = [1, 2, 3, 4].map(AccountId::from_u128).into();
        let found = ProfileCoachIntroductionLookup::new(accounts)
            .find_coach_introductions(&ids)
            .await
            .unwrap();

        assert_eq!(
            found,
            BTreeMap::from([
                (
                    AccountId::from_u128(1),
                    CoachIntroduction {
                        display_name: DisplayName::new("たなか").unwrap(),
                        bio,
                    }
                ),
                (
                    AccountId::from_u128(3),
                    CoachIntroduction {
                        display_name: DisplayName::new("いとう").unwrap(),
                        bio: CoachBio::default(),
                    }
                ),
            ]),
            "生徒（2）とないアカウント（4）は含めない。自己紹介文を書いていなければ空"
        );
    }

    #[tokio::test]
    async fn no_ids_is_an_empty_map() {
        let found = ProfileCoachIntroductionLookup::new(InMemoryAccountRepository::default())
            .find_coach_introductions(&BTreeSet::new())
            .await
            .unwrap();
        assert!(found.is_empty());
    }
}
