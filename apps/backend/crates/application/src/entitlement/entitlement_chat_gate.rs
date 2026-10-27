use domain::identity::AccountId;

use super::EntitlementRepository;
use crate::{Unavailable, messaging::ChatGate};

/// messaging の [`ChatGate`] の entitlement による実装。生徒の受講権が、動画とチャットの送り放題
/// （domain の [`domain::entitlement::Entitlement::unlimited_video_and_chat`]。月額だけ）を持つかで答える。
///
/// 読むだけで何も保存しない（送り放題なので回数を消費しない）。受講権のない生徒（月額契約の前）とワンショットの
/// 生徒は送れない。
#[derive(Debug, Clone)]
pub struct EntitlementChatGate<E> {
    entitlements: E,
}

impl<E> EntitlementChatGate<E> {
    pub fn new(entitlements: E) -> Self {
        Self { entitlements }
    }
}

impl<E> ChatGate for EntitlementChatGate<E>
where
    E: EntitlementRepository,
{
    async fn may_chat(&self, student_id: AccountId) -> Result<bool, Unavailable> {
        Ok(self
            .entitlements
            .find_by_student(student_id)
            .await?
            .is_some_and(|entitlement| entitlement.unlimited_video_and_chat()))
    }
}

#[cfg(test)]
mod tests {
    use domain::{
        entitlement::{Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, YearMonth},
        identity::Sport,
    };

    use super::*;
    use crate::in_memory::InMemoryEntitlementRepository;

    const MONTHLY: AccountId = AccountId::from_u128(1);
    const ONE_SHOT: AccountId = AccountId::from_u128(2);
    const NONE: AccountId = AccountId::from_u128(3);

    fn gate() -> EntitlementChatGate<InMemoryEntitlementRepository> {
        let october = YearMonth::new(2026, 10).unwrap();
        EntitlementChatGate::new(InMemoryEntitlementRepository::with_entitlements([
            Entitlement::monthly(
                EntitlementId::from_u128(10),
                MONTHLY,
                Sport::Tennis,
                SubscriptionId::from_u128(20),
                LiveLessonQuota::fresh(october),
            )
            .unwrap(),
            Entitlement::one_shot(EntitlementId::from_u128(11), ONE_SHOT, Sport::Golf, 1).unwrap(),
        ]))
    }

    #[tokio::test]
    async fn only_monthly_students_may_chat() {
        let gate = gate();
        assert!(gate.may_chat(MONTHLY).await.unwrap());
        assert!(!gate.may_chat(ONE_SHOT).await.unwrap());
        assert!(
            !gate.may_chat(NONE).await.unwrap(),
            "月額契約の前は送れない"
        );
    }

    #[tokio::test]
    async fn asking_consumes_nothing() {
        let gate = gate();
        let before = gate.entitlements.all();
        for _ in 0..3 {
            gate.may_chat(MONTHLY).await.unwrap();
        }
        assert_eq!(gate.entitlements.all(), before);
    }
}
