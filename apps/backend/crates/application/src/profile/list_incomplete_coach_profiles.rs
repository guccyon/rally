use domain::{
    catalog::Sport,
    profile::{CoachSportFilter, MissingCoachProfileFields},
};

use super::{CoachProfileRepository, IncompleteCoachProfile};
use crate::{Unavailable, identity::AccountRepository};

/// プロフィールに足りない項目のあるコーチの一覧のユースケース（admin-web の `listIncompleteCoachProfiles`。本部だけが
/// 呼ぶ）。本部が自己紹介の書きかけのコーチを見つけて声をかけるのに使う。
///
/// すべてのコーチのアカウント（受付を止めているコーチも含む）のうち、足りない項目（domain の
/// [`MissingCoachProfileFields`] で判定する）が 1 つ以上あるものを、アカウントを登録した時刻の古い順（同じ時刻は ID の順）で
/// 返す。任意の競技 `sport` で絞れる（コーチの提供スポーツに含むものだけ。判定は domain の [`CoachSportFilter`] で、
/// `listCoaches` の `sport` と同じ）。`None` なら絞らない。絞っても「足りない」の判定と並べ方は変わらない。
/// プロフィールを最後に更新した時刻は、今の保存先が持たないので常に `None`（記録するのは別の段）。
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

    pub async fn execute(
        &self,
        sport: Option<Sport>,
    ) -> Result<Vec<IncompleteCoachProfile>, Unavailable> {
        let filter = CoachSportFilter::new(sport);
        let bios = self.profiles.list_coach_bios().await?;
        let mut incomplete: Vec<IncompleteCoachProfile> = self
            .accounts
            .list_coaches()
            .await?
            .into_iter()
            .filter(|account| filter.matches(account))
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
        account_with_sports(id, role, sports, registered_hour)
    }

    fn account_with_sports(
        id: u128,
        role: Role,
        sports: &[Sport],
        registered_hour: u64,
    ) -> Account {
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
        listed_in(accounts, None).await
    }

    async fn listed_in(
        accounts: &InMemoryAccountRepository,
        sport: Option<Sport>,
    ) -> Vec<IncompleteCoachProfile> {
        ListIncompleteCoachProfiles::new(accounts.clone(), accounts.clone())
            .execute(sport)
            .await
            .unwrap()
    }

    fn ids(profiles: &[IncompleteCoachProfile]) -> Vec<u128> {
        profiles.iter().map(|p| p.account.id().as_u128()).collect()
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

    #[tokio::test]
    async fn sport_lists_only_coaches_offering_it_keeping_the_order() {
        let accounts = accounts([
            account_with_sports(1, Role::Headquarters, &[], 0),
            account_with_sports(2, Role::Student, &[Sport::Golf], 0),
            account_with_sports(3, Role::Coach, &[Sport::Tennis, Sport::Golf], 4),
            account_with_sports(4, Role::Coach, &[Sport::Golf], 1),
            account_with_sports(5, Role::Coach, &[Sport::Golf], 2),
            account_with_sports(6, Role::Coach, &[Sport::Soccer], 0),
            account_with_sports(7, Role::Coach, &[Sport::Golf], 3),
        ])
        .await;
        // 5 はゴルフのコーチでも書き終えているので載らない
        write_bio(&accounts, 5, "ゴルフ歴 10 年").await;
        write_bio(&accounts, 7, "\n").await;

        assert_eq!(
            ids(&listed_in(&accounts, Some(Sport::Golf)).await),
            [4, 7, 3],
            "提供スポーツのどれかがゴルフのコーチだけ。登録の古い順のまま。生徒・本部は載らない"
        );
        assert_eq!(ids(&listed_in(&accounts, Some(Sport::Tennis)).await), [3]);
        assert_eq!(ids(&listed_in(&accounts, Some(Sport::Soccer)).await), [6]);
        assert!(
            listed_in(&accounts, Some(Sport::Swimming)).await.is_empty(),
            "合うコーチがいなければ空"
        );
        assert_eq!(
            ids(&listed(&accounts).await),
            [6, 4, 7, 3],
            "付けなければ今までどおり"
        );
    }
}
