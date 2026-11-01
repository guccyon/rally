use std::collections::BTreeSet;

use domain::{catalog::Sport, identity::AccountId, scheduling::CoachAcceptanceStatus};

use super::{CoachAcceptanceLookup, ListedCoach};
use crate::{Unavailable, identity::AccountRepository};

/// コーチの一覧のユースケース（admin-web の `listCoaches`。本部だけが呼ぶ）。
///
/// コーチのアカウントを表示名の順（Unicode のコードポイントの順。同じ表示名は ID の順）で、1 件ごとに受付の状態
/// （scheduling に問い合わせる。[`CoachAcceptanceLookup`]）を添えて返す。受付を止めているコーチも載せる。
/// 任意の条件で絞れる: 受付の状態（`acceptance`）・競技（`sport`。コーチの提供スポーツに含むもの）。両方なら両方を満たすもの、
/// どちらも `None` なら絞らない。並べ方は絞っても変わらない。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct ListCoaches<R, A> {
    accounts: R,
    acceptances: A,
}

impl<R, A> ListCoaches<R, A>
where
    R: AccountRepository,
    A: CoachAcceptanceLookup,
{
    pub fn new(accounts: R, acceptances: A) -> Self {
        Self {
            accounts,
            acceptances,
        }
    }

    pub async fn execute(
        &self,
        acceptance: Option<CoachAcceptanceStatus>,
        sport: Option<Sport>,
    ) -> Result<Vec<ListedCoach>, Unavailable> {
        let coaches: Vec<_> = self
            .accounts
            .list_coaches()
            .await?
            .into_iter()
            .filter(|account| {
                sport
                    .is_none_or(|sport| account.as_coach().is_some_and(|coach| coach.offers(sport)))
            })
            .collect();
        let coach_ids: BTreeSet<AccountId> = coaches.iter().map(|account| account.id()).collect();
        let acceptances = if coach_ids.is_empty() {
            Default::default()
        } else {
            self.acceptances.find_coach_acceptances(&coach_ids).await?
        };
        Ok(coaches
            .into_iter()
            .map(|account| ListedCoach {
                acceptance: acceptances
                    .get(&account.id())
                    .copied()
                    .unwrap_or(CoachAcceptanceStatus::Accepting),
                account,
            })
            .filter(|coach| acceptance.is_none_or(|status| coach.acceptance == status))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use domain::{
        identity::{Account, Email, Role},
        profile::DisplayName,
    };

    use super::*;
    use crate::{
        in_memory::{InMemoryAccountRepository, InMemoryCoachAcceptanceRepository},
        scheduling::{CoachAcceptanceRepository, SchedulingCoachAcceptanceLookup},
    };

    /// 受付を止める・再開する本部のアカウント。
    const HEADQUARTERS_ID: AccountId = AccountId::from_u128(900);

    type Acceptances = SchedulingCoachAcceptanceLookup<InMemoryCoachAcceptanceRepository>;

    fn no_suspension() -> Acceptances {
        SchedulingCoachAcceptanceLookup::new(InMemoryCoachAcceptanceRepository::default())
    }

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

        let coaches = ListCoaches::new(accounts, no_suspension())
            .execute(None, None)
            .await
            .unwrap();
        let listed: Vec<_> = coaches
            .iter()
            .map(|c| (c.account.id().as_u128(), c.account.display_name().as_str()))
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

        let coaches = ListCoaches::new(accounts, no_suspension())
            .execute(None, None)
            .await
            .unwrap();
        assert!(coaches.is_empty());
    }

    #[tokio::test]
    async fn each_coach_has_its_acceptance_status_and_suspended_coaches_are_listed() {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "いとう", Role::Coach, &[Sport::Soccer]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        acceptances
            .update(AccountId::from_u128(1), |a| {
                a.suspend(HEADQUARTERS_ID, SystemTime::UNIX_EPOCH)
            })
            .await
            .unwrap();

        let coaches = ListCoaches::new(accounts, SchedulingCoachAcceptanceLookup::new(acceptances))
            .execute(None, None)
            .await
            .unwrap();
        let listed: Vec<_> = coaches
            .iter()
            .map(|c| (c.account.id().as_u128(), c.acceptance))
            .collect();
        assert_eq!(
            listed,
            [
                (2, CoachAcceptanceStatus::Accepting),
                (1, CoachAcceptanceStatus::Suspended)
            ]
        );
    }

    /// たなか（1。テニス・停止中）・いとう（2。サッカーとテニス）・うえの（3。野球・停止中）と、テニスの生徒（4）。
    async fn filtered(
        acceptance: Option<CoachAcceptanceStatus>,
        sport: Option<Sport>,
    ) -> Vec<(u128, CoachAcceptanceStatus)> {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(1, "たなか", Role::Coach, &[Sport::Tennis]),
            account(2, "いとう", Role::Coach, &[Sport::Soccer, Sport::Tennis]),
            account(3, "うえの", Role::Coach, &[Sport::Baseball]),
            account(4, "あおき", Role::Student, &[Sport::Tennis]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        for id in [1, 3] {
            acceptances
                .update(AccountId::from_u128(id), |a| {
                    a.suspend(HEADQUARTERS_ID, SystemTime::UNIX_EPOCH)
                })
                .await
                .unwrap();
        }
        ListCoaches::new(accounts, SchedulingCoachAcceptanceLookup::new(acceptances))
            .execute(acceptance, sport)
            .await
            .unwrap()
            .iter()
            .map(|c| (c.account.id().as_u128(), c.acceptance))
            .collect()
    }

    #[tokio::test]
    async fn filters_by_acceptance_status_keeping_the_order() {
        use CoachAcceptanceStatus::{Accepting, Suspended};
        assert_eq!(
            filtered(Some(Suspended), None).await,
            [(3, Suspended), (1, Suspended)]
        );
        assert_eq!(filtered(Some(Accepting), None).await, [(2, Accepting)]);
    }

    #[tokio::test]
    async fn filters_by_sport_the_coach_offers() {
        use CoachAcceptanceStatus::{Accepting, Suspended};
        assert_eq!(
            filtered(None, Some(Sport::Tennis)).await,
            [(2, Accepting), (1, Suspended)],
            "提供スポーツのどれかが一致すれば載る。テニスの生徒は載らない"
        );
        assert!(filtered(None, Some(Sport::Golf)).await.is_empty());
    }

    #[tokio::test]
    async fn both_conditions_must_hold_and_no_condition_lists_all() {
        use CoachAcceptanceStatus::{Accepting, Suspended};
        assert_eq!(
            filtered(Some(Suspended), Some(Sport::Tennis)).await,
            [(1, Suspended)]
        );
        assert!(
            filtered(Some(Accepting), Some(Sport::Baseball))
                .await
                .is_empty()
        );
        assert_eq!(
            filtered(None, None).await,
            [(2, Accepting), (3, Suspended), (1, Suspended)]
        );
    }
}
