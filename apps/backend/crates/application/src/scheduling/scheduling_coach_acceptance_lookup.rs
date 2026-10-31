use std::collections::{BTreeMap, BTreeSet};

use domain::{identity::AccountId, scheduling::CoachAcceptanceStatus};

use super::CoachAcceptanceRepository;
use crate::{Unavailable, matching::SuspendedCoachLookup, profile::CoachAcceptanceLookup};

/// matching の [`SuspendedCoachLookup`] と profile の [`CoachAcceptanceLookup`] の scheduling による実装。コーチの受付の
/// 保存先から停止中のコーチを読むだけで、何も変えない。
#[derive(Debug, Clone)]
pub struct SchedulingCoachAcceptanceLookup<R> {
    acceptances: R,
}

impl<R> SchedulingCoachAcceptanceLookup<R> {
    pub fn new(acceptances: R) -> Self {
        Self { acceptances }
    }
}

impl<R> SuspendedCoachLookup for SchedulingCoachAcceptanceLookup<R>
where
    R: CoachAcceptanceRepository,
{
    async fn find_suspended_coaches(&self) -> Result<BTreeSet<AccountId>, Unavailable> {
        self.acceptances.suspended_coaches().await
    }
}

impl<R> CoachAcceptanceLookup for SchedulingCoachAcceptanceLookup<R>
where
    R: CoachAcceptanceRepository,
{
    async fn find_coach_acceptances(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, CoachAcceptanceStatus>, Unavailable> {
        let suspended = self.acceptances.suspended_coaches().await?;
        Ok(coach_ids
            .iter()
            .map(|id| {
                let status = if suspended.contains(id) {
                    CoachAcceptanceStatus::Suspended
                } else {
                    CoachAcceptanceStatus::Accepting
                };
                (*id, status)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use crate::in_memory::InMemoryCoachAcceptanceRepository;

    /// 受付を止める・再開する本部のアカウント。
    const HEADQUARTERS_ID: AccountId = AccountId::from_u128(900);

    const SUSPENDED: AccountId = AccountId::from_u128(7);
    const ACCEPTING: AccountId = AccountId::from_u128(8);

    async fn lookup() -> SchedulingCoachAcceptanceLookup<InMemoryCoachAcceptanceRepository> {
        let acceptances = InMemoryCoachAcceptanceRepository::default();
        acceptances
            .update(SUSPENDED, |a| {
                a.suspend(HEADQUARTERS_ID, SystemTime::UNIX_EPOCH)
            })
            .await
            .unwrap();
        SchedulingCoachAcceptanceLookup::new(acceptances)
    }

    #[tokio::test]
    async fn finds_suspended_coaches() {
        assert_eq!(
            lookup().await.find_suspended_coaches().await.unwrap(),
            BTreeSet::from([SUSPENDED])
        );
    }

    #[tokio::test]
    async fn every_asked_coach_has_a_status_and_unsuspended_coaches_are_accepting() {
        let found = lookup()
            .await
            .find_coach_acceptances(&BTreeSet::from([SUSPENDED, ACCEPTING]))
            .await
            .unwrap();
        assert_eq!(
            found,
            BTreeMap::from([
                (SUSPENDED, CoachAcceptanceStatus::Suspended),
                (ACCEPTING, CoachAcceptanceStatus::Accepting),
            ])
        );
    }
}
