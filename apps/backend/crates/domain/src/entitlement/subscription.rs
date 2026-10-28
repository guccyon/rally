use std::time::SystemTime;

use super::{ContractType, SubscriptionId, SubscriptionPlan, SubscriptionStatus};
use crate::{DomainError, identity::AccountId, identity::Sport};

/// 月額の契約（glossary の subscription）。ゴルフ以外のスポーツで結ぶ。
///
/// 契約の開始は application の `entitlement::StartSubscription`（決済は billing で、Gen1 の範囲外）。ここでは型と、
/// 生成時の不変条件（スポーツ）と、ビデオ通話の暦月の上限を決めるプラン（[`SubscriptionPlan`]）を持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    id: SubscriptionId,
    student_id: AccountId,
    sport: Sport,
    plan: SubscriptionPlan,
    status: SubscriptionStatus,
    started_at: SystemTime,
}

impl Subscription {
    /// `sport` がゴルフなら [`DomainError::SportNotCoveredByContract`]（月額はゴルフ以外）。
    pub fn new(
        id: SubscriptionId,
        student_id: AccountId,
        sport: Sport,
        plan: SubscriptionPlan,
        status: SubscriptionStatus,
        started_at: SystemTime,
    ) -> Result<Self, DomainError> {
        ContractType::Monthly.ensure_covers(sport)?;
        Ok(Self {
            id,
            student_id,
            sport,
            plan,
            status,
            started_at,
        })
    }

    pub const fn id(&self) -> SubscriptionId {
        self.id
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub const fn sport(&self) -> Sport {
        self.sport
    }

    pub const fn plan(&self) -> SubscriptionPlan {
        self.plan
    }

    pub const fn status(&self) -> SubscriptionStatus {
        self.status
    }

    pub const fn started_at(&self) -> SystemTime {
        self.started_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subscription(sport: Sport) -> Result<Subscription, DomainError> {
        Subscription::new(
            SubscriptionId::from_u128(1),
            AccountId::from_u128(2),
            sport,
            SubscriptionPlan::Plus,
            SubscriptionStatus::Active,
            SystemTime::UNIX_EPOCH,
        )
    }

    #[test]
    fn subscribes_to_sports_other_than_golf() {
        let subscription = subscription(Sport::Tennis).unwrap();
        assert_eq!(subscription.sport(), Sport::Tennis);
        assert_eq!(subscription.plan(), SubscriptionPlan::Plus);
        assert_eq!(subscription.status(), SubscriptionStatus::Active);
        assert_eq!(subscription.started_at(), SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn rejects_golf() {
        assert_eq!(
            subscription(Sport::Golf),
            Err(DomainError::SportNotCoveredByContract {
                contract_type: ContractType::Monthly,
                sport: Sport::Golf,
            })
        );
    }
}
