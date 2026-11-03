use std::collections::BTreeSet;

use domain::{
    catalog::Sport,
    identity::AccountId,
    profile::{
        CoachAcceptanceFilter, CoachSportFilter, DisplayNameQuery, MissingCoachProfileFields,
    },
    scheduling::CoachAcceptanceStatus,
};

use super::{CoachAcceptanceLookup, CoachProfileRepository, IncompleteCoachProfile};
use crate::{Unavailable, identity::AccountRepository};

/// プロフィールに足りない項目のあるコーチの一覧のユースケース（admin-web の `listIncompleteCoachProfiles`。本部だけが
/// 呼ぶ）。本部が自己紹介の書きかけのコーチを見つけて声をかけるのに使う。
///
/// すべてのコーチのアカウント（受付を止めているコーチも含む）のうち、足りない項目（domain の
/// [`MissingCoachProfileFields`] で判定する）が 1 つ以上あるものを、アカウントを登録した時刻の古い順（同じ時刻は ID の順）で
/// 返す。任意の競技 `sport` で絞れる（コーチの提供スポーツに含むものだけ。判定は domain の [`CoachSportFilter`] で、
/// `listCoaches` の `sport` と同じ）。`None` なら絞らない。任意の受付の状態 `acceptance` でも絞れる（scheduling に問い合わせる。
/// [`CoachAcceptanceLookup`]。判定は domain の [`CoachAcceptanceFilter`] で、`listCoaches` の `acceptance_status` と同じ。
/// 止めていないコーチは受付中）。`None` なら絞らず、問い合わせもしない。任意の表示名の検索語 `name` でも絞れる（表示名に
/// 部分一致するものだけ。一致の規則は domain の [`DisplayNameQuery`] で、`listCoaches` の `q` と同じ。ここでは比べ方を
/// 持たない）。`None` なら絞らない。付けたものをすべて満たすものだけ。
/// 絞っても「足りない」の判定と並べ方は変わらない。
/// プロフィールを最後に更新した時刻は、今の保存先が持たないので常に `None`（記録するのは別の段）。
/// 照会だけで、行動イベントも境界間イベントも発火しない。誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListIncompleteCoachProfiles<R, P, A> {
    accounts: R,
    profiles: P,
    acceptances: A,
}

impl<R, P, A> ListIncompleteCoachProfiles<R, P, A>
where
    R: AccountRepository,
    P: CoachProfileRepository,
    A: CoachAcceptanceLookup,
{
    pub fn new(accounts: R, profiles: P, acceptances: A) -> Self {
        Self {
            accounts,
            profiles,
            acceptances,
        }
    }

    pub async fn execute(
        &self,
        sport: Option<Sport>,
        acceptance: Option<CoachAcceptanceStatus>,
        name: Option<&DisplayNameQuery>,
    ) -> Result<Vec<IncompleteCoachProfile>, Unavailable> {
        let filter = CoachSportFilter::new(sport);
        let bios = self.profiles.list_coach_bios().await?;
        let mut incomplete: Vec<IncompleteCoachProfile> = self
            .accounts
            .list_coaches()
            .await?
            .into_iter()
            .filter(|account| filter.matches(account))
            .filter(|account| name.is_none_or(|name| name.matches(account.display_name())))
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
        let acceptance = CoachAcceptanceFilter::new(acceptance);
        if acceptance.status().is_some() && !incomplete.is_empty() {
            let coach_ids: BTreeSet<AccountId> = incomplete
                .iter()
                .map(|profile| profile.account.id())
                .collect();
            let statuses = self.acceptances.find_coach_acceptances(&coach_ids).await?;
            incomplete.retain(|profile| {
                acceptance.matches(
                    statuses
                        .get(&profile.account.id())
                        .copied()
                        .unwrap_or(CoachAcceptanceStatus::Accepting),
                )
            });
        }
        incomplete.sort_by_key(|profile| (profile.account.registered_at(), profile.account.id()));
        Ok(incomplete)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        identity::{Account, Email, Role},
        profile::{CoachBio, CoachProfileField, DisplayName, DisplayNameQuery},
        scheduling::CoachSuspensionReason,
    };

    use super::*;
    use crate::{
        in_memory::{InMemoryAccountRepository, InMemoryCoachAcceptanceRepository},
        scheduling::{CoachAcceptanceRepository, SchedulingCoachAcceptanceLookup},
    };

    /// 受付を止める本部のアカウント。
    const HEADQUARTERS_ID: AccountId = AccountId::from_u128(900);

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
        named(id, role, &format!("なまえ{id}"), sports, registered_hour)
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
        listed_with(
            accounts,
            &InMemoryCoachAcceptanceRepository::default(),
            sport,
            None,
        )
        .await
    }

    async fn listed_with(
        accounts: &InMemoryAccountRepository,
        acceptances: &InMemoryCoachAcceptanceRepository,
        sport: Option<Sport>,
        acceptance: Option<CoachAcceptanceStatus>,
    ) -> Vec<IncompleteCoachProfile> {
        listed_by(accounts, acceptances, sport, acceptance, None).await
    }

    async fn listed_by(
        accounts: &InMemoryAccountRepository,
        acceptances: &InMemoryCoachAcceptanceRepository,
        sport: Option<Sport>,
        acceptance: Option<CoachAcceptanceStatus>,
        name: Option<&str>,
    ) -> Vec<IncompleteCoachProfile> {
        let name = name.map(|name| DisplayNameQuery::new(name).unwrap().unwrap());
        ListIncompleteCoachProfiles::new(
            accounts.clone(),
            accounts.clone(),
            SchedulingCoachAcceptanceLookup::new(acceptances.clone()),
        )
        .execute(sport, acceptance, name.as_ref())
        .await
        .unwrap()
    }

    fn named(id: u128, role: Role, name: &str, sports: &[Sport], registered_hour: u64) -> Account {
        Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new(name).unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(registered_hour * 3600),
        )
        .unwrap()
    }

    async fn suspend(acceptances: &InMemoryCoachAcceptanceRepository, id: u128) {
        acceptances
            .update(AccountId::from_u128(id), |a| {
                a.suspend(
                    HEADQUARTERS_ID,
                    CoachSuspensionReason::default(),
                    SystemTime::UNIX_EPOCH,
                )
            })
            .await
            .unwrap();
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

    #[tokio::test]
    async fn acceptance_lists_only_coaches_in_that_status_keeping_the_order() {
        let accounts = accounts([
            account_with_sports(1, Role::Headquarters, &[], 0),
            account_with_sports(2, Role::Coach, &[Sport::Tennis], 3),
            account_with_sports(3, Role::Coach, &[Sport::Golf], 1),
            account_with_sports(4, Role::Coach, &[Sport::Tennis, Sport::Golf], 2),
            account_with_sports(5, Role::Coach, &[Sport::Tennis], 0),
            account_with_sports(6, Role::Coach, &[Sport::Soccer], 4),
        ])
        .await;
        // 6 は受付中でも書き終えているので載らない
        write_bio(&accounts, 6, "サッカー歴 5 年").await;
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        suspend(&acceptances, 3).await;
        suspend(&acceptances, 5).await;
        // 止めてから再開したコーチは受付中
        suspend(&acceptances, 4).await;
        acceptances
            .update(AccountId::from_u128(4), |a| {
                a.resume(HEADQUARTERS_ID, SystemTime::UNIX_EPOCH)
            })
            .await
            .unwrap();

        let listed = |sport, acceptance| listed_with(&accounts, &acceptances, sport, acceptance);
        assert_eq!(
            ids(&listed(None, Some(CoachAcceptanceStatus::Accepting)).await),
            [4, 2],
            "受付中（止めたことのないコーチ・再開したコーチ）だけ。登録の古い順のまま"
        );
        assert_eq!(
            ids(&listed(None, Some(CoachAcceptanceStatus::Suspended)).await),
            [5, 3]
        );
        assert_eq!(
            ids(&listed(Some(Sport::Golf), Some(CoachAcceptanceStatus::Accepting)).await),
            [4],
            "競技と組み合わせれば両方を満たすものだけ"
        );
        assert_eq!(
            ids(&listed(Some(Sport::Tennis), Some(CoachAcceptanceStatus::Suspended)).await),
            [5]
        );
        assert!(
            listed(Some(Sport::Soccer), Some(CoachAcceptanceStatus::Accepting))
                .await
                .is_empty(),
            "合うコーチがいなければ空"
        );
        assert_eq!(
            ids(&listed(None, None).await),
            [5, 3, 4, 2],
            "付けなければ今までどおり（止めているコーチも載る）"
        );
    }

    #[tokio::test]
    async fn name_lists_only_coaches_whose_display_name_contains_it_keeping_the_order() {
        let accounts = accounts([
            named(1, Role::Headquarters, "山田 本部", &[], 0),
            named(2, Role::Student, "山田 生徒", &[Sport::Tennis], 0),
            named(3, Role::Coach, "山田 太郎", &[Sport::Tennis], 3),
            named(4, Role::Coach, "Hanako YAMADA", &[Sport::Golf], 1),
            named(5, Role::Coach, "小山田 次郎", &[Sport::Tennis], 2),
            named(6, Role::Coach, "山田 書き終え", &[Sport::Tennis], 0),
            named(7, Role::Coach, "佐藤 花子", &[Sport::Tennis], 0),
        ])
        .await;
        // 6 は名前が合っても書き終えているので載らない
        write_bio(&accounts, 6, "テニス歴 3 年").await;
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        suspend(&acceptances, 5).await;

        let listed =
            |sport, acceptance, name| listed_by(&accounts, &acceptances, sport, acceptance, name);
        assert_eq!(
            ids(&listed(None, None, Some("山田")).await),
            [5, 3],
            "表示名に含むコーチだけ。登録の古い順のまま。本部・生徒・書き終えたコーチは載らない"
        );
        assert_eq!(
            ids(&listed(None, None, Some(" yamada\u{3000}")).await),
            [4],
            "前後の空白を除き、ASCII の英字は大文字と小文字を区別しない（規則は DisplayNameQuery）"
        );
        assert_eq!(
            ids(&listed(Some(Sport::Tennis), None, Some("山田")).await),
            [5, 3],
            "競技と組み合わせられる"
        );
        assert_eq!(
            ids(&listed(None, Some(CoachAcceptanceStatus::Accepting), Some("山田")).await),
            [3],
            "受付の状態と組み合わせられる"
        );
        assert_eq!(
            ids(&listed(
                Some(Sport::Golf),
                Some(CoachAcceptanceStatus::Accepting),
                Some("YAMADA")
            )
            .await),
            [4],
            "3 つとも付ければすべてを満たすものだけ"
        );
        assert!(
            listed(None, None, Some("鈴木")).await.is_empty(),
            "合うコーチがいなければ空"
        );
        assert_eq!(
            ids(&listed(None, None, None).await),
            [7, 4, 5, 3],
            "付けなければ今までどおり"
        );
    }
}
