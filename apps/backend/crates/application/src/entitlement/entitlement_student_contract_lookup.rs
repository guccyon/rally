use std::collections::{BTreeMap, BTreeSet};

use domain::identity::AccountId;

use super::EntitlementRepository;
use crate::{
    Unavailable,
    identity::{StudentContract, StudentContractLookup},
};

/// identity の [`StudentContractLookup`] の entitlement による実装。受講権の保存先から読むだけで、何も変えない。
///
/// 受講権がない生徒（まだ契約していない生徒）は結果に含めない。
#[derive(Debug, Clone)]
pub struct EntitlementStudentContractLookup<E> {
    entitlements: E,
}

impl<E> EntitlementStudentContractLookup<E> {
    pub fn new(entitlements: E) -> Self {
        Self { entitlements }
    }
}

impl<E> StudentContractLookup for EntitlementStudentContractLookup<E>
where
    E: EntitlementRepository,
{
    async fn find_student_contracts(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, StudentContract>, Unavailable> {
        let mut contracts = BTreeMap::new();
        for &student_id in student_ids {
            let Some(entitlement) = self.entitlements.find_by_student(student_id).await? else {
                continue;
            };
            contracts.insert(
                student_id,
                StudentContract {
                    contract_type: entitlement.contract_type(),
                    plan: entitlement.plan(),
                },
            );
        }
        Ok(contracts)
    }
}

#[cfg(test)]
mod tests {
    use domain::{
        catalog::Sport,
        entitlement::{
            ContractType, Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId,
            SubscriptionPlan, YearMonth,
        },
    };

    use super::*;
    use crate::in_memory::InMemoryEntitlementRepository;

    const MONTHLY_STUDENT: AccountId = AccountId::from_u128(1);
    const GOLF_STUDENT: AccountId = AccountId::from_u128(2);
    const NO_CONTRACT_STUDENT: AccountId = AccountId::from_u128(3);
    const NOT_ASKED_STUDENT: AccountId = AccountId::from_u128(4);

    fn monthly(id: u128, student_id: AccountId, plan: SubscriptionPlan) -> Entitlement {
        Entitlement::monthly(
            EntitlementId::from_u128(id),
            student_id,
            Sport::Tennis,
            SubscriptionId::from_u128(id),
            LiveLessonQuota::restore(plan, YearMonth::new(2026, 10).unwrap(), []).unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn returns_contract_type_and_plan_of_students_with_entitlement() {
        let entitlements = InMemoryEntitlementRepository::with_entitlements([
            monthly(10, MONTHLY_STUDENT, SubscriptionPlan::Plus),
            Entitlement::one_shot(EntitlementId::from_u128(11), GOLF_STUDENT, Sport::Golf, 1)
                .unwrap(),
            monthly(12, NOT_ASKED_STUDENT, SubscriptionPlan::Standard),
        ]);
        let lookup = EntitlementStudentContractLookup::new(entitlements);

        let ids = [MONTHLY_STUDENT, GOLF_STUDENT, NO_CONTRACT_STUDENT].into();
        assert_eq!(
            lookup.find_student_contracts(&ids).await.unwrap(),
            BTreeMap::from([
                (
                    MONTHLY_STUDENT,
                    StudentContract {
                        contract_type: ContractType::Monthly,
                        plan: Some(SubscriptionPlan::Plus),
                    }
                ),
                (
                    GOLF_STUDENT,
                    StudentContract {
                        contract_type: ContractType::OneShot,
                        plan: None,
                    }
                ),
            ]),
            "受講権のない生徒と、問い合わせていない生徒は含めない"
        );
    }
}
