use domain::profile::MissingCoachProfileFields;

use super::{CoachProfileRepository, IncompleteCoachProfile};
use crate::{Unavailable, identity::AccountRepository};

/// プロフィールに足りない項目のあるコーチの一覧のユースケース（admin-web の `listIncompleteCoachProfiles`。本部だけが
/// 呼ぶ）。本部が自己紹介の書きかけのコーチを見つけて声をかけるのに使う。
///
/// すべてのコーチのアカウント（受付を止めているコーチも含む）のうち、足りない項目（domain の
/// [`MissingCoachProfileFields`] で判定する）が 1 つ以上あるものを、アカウントを登録した時刻の古い順（同じ時刻は ID の順）で
/// 返す。プロフィールを最後に更新した時刻は、今の保存先が持たないので常に `None`（記録するのは別の段）。
/// 照会だけで、行動イベントも境界間イベントも発火しない。誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListIncompleteCoachProfiles<R, P> {
    accounts: R,
    profiles: P,
}

impl<R, P> ListIncompleteCoachProfiles<R, P>
where
    R: AccountRepository,
    P: CoachProfileRepository,
{
    pub fn new(accounts: R, profiles: P) -> Self {
        Self { accounts, profiles }
    }

    pub async fn execute(&self) -> Result<Vec<IncompleteCoachProfile>, Unavailable> {
        let bios = self.profiles.list_coach_bios().await?;
        let mut incomplete: Vec<IncompleteCoachProfile> = self
            .accounts
            .list_coaches()
            .await?
            .into_iter()
            .filter_map(|account| {
                // 自己紹介文の行がないコーチは書いていない（空）とみなす
                let bio = bios.get(&account.id()).cloned().unwrap_or_default();
                let missing_fields = MissingCoachProfileFields::of(&bio);
                (!missing_fields.is_empty()).then_some(IncompleteCoachProfile {
                    account,
                    missing_fields,
                    updated_at: None,
                })
            })
            .collect();
        incomplete.sort_by_key(|profile| (profile.account.registered_at(), profile.account.id()));
        Ok(incomplete)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        identity::{Account, AccountId, Email, Role},
        profile::{CoachBio, CoachProfileField, DisplayName},
    };

    use super::*;
    use crate::in_memory::InMemoryAccountRepository;

    fn account(id: u128, role: Role, registered_hour: u64) -> Account {
        let sports: &[Sport] = match role {
            Role::Headquarters => &[],
            _ => &[Sport::Tennis],
        };
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new(format!("なまえ{id}")).unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(registered_hour * 3600),
        )
        .unwrap()
    }

    async fn accounts(accounts: impl IntoIterator<Item = Account>) -> InMemoryAccountRepository {
        let repository = InMemoryAccountRepository::default();
        for account in accounts {
            repository.insert(account).await.unwrap();
        }
        repository
    }

    async fn write_bio(accounts: &InMemoryAccountRepository, id: u128, bio: &str) {
        accounts
            .update_coach_bio(AccountId::from_u128(id), CoachBio::new(bio).unwrap())
            .await
            .unwrap()
            .unwrap();
    }

    async fn listed(accounts: &InMemoryAccountRepository) -> Vec<IncompleteCoachProfile> {
        ListIncompleteCoachProfiles::new(accounts.clone(), accounts.clone())
            .execute()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn lists_only_coaches_with_missing_fields_oldest_registration_first() {
        let accounts = accounts([
            account(1, Role::Headquarters, 0),
            account(2, Role::Coach, 5),
            account(3, Role::Coach, 1),
            account(4, Role::Student, 0),
            account(5, Role::Coach, 2),
            account(6, Role::Coach, 3),
            // 同じ登録時刻は ID の順
            account(8, Role::Coach, 1),
            account(7, Role::Coach, 1),
        ])
        .await;
        // 5 は書き終えている。6 は空白だけ。2・3・7・8 は書いていない
        write_bio(&accounts, 5, "テニス歴 20 年").await;
        write_bio(&accounts, 6, " \u{3000}").await;

        let profiles = listed(&accounts).await;
        let ids: Vec<u128> = profiles.iter().map(|p| p.account.id().as_u128()).collect();
        assert_eq!(
            ids,
            [3, 7, 8, 6, 2],
            "本部・生徒と、書き終えたコーチは載らない"
        );
        for profile in &profiles {
            assert_eq!(profile.missing_fields.fields(), [CoachProfileField::Bio]);
            assert_eq!(profile.updated_at, None, "更新の時刻はまだ記録していない");
        }
    }

    #[tokio::test]
    async fn coach_who_cleared_the_bio_is_listed_again() {
        let accounts = accounts([account(2, Role::Coach, 0)]).await;
        write_bio(&accounts, 2, "よろしく").await;
        assert!(listed(&accounts).await.is_empty());

        write_bio(&accounts, 2, "").await;
        let profiles = listed(&accounts).await;
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].account.id(), AccountId::from_u128(2));
    }

    #[tokio::test]
    async fn no_incomplete_coaches_is_an_empty_list() {
        assert!(listed(&accounts([]).await).await.is_empty());
        let accounts = accounts([
            account(1, Role::Headquarters, 0),
            account(4, Role::Student, 0),
        ])
        .await;
        assert!(listed(&accounts).await.is_empty());
    }
}
