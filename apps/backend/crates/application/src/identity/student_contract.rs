use domain::entitlement::{ContractType, SubscriptionPlan};

/// 生徒の一覧（[`super::ListStudents`]）に添える、生徒の受講権の契約種別と契約プラン。entitlement から写したもので、
/// identity は読むだけ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StudentContract {
    pub contract_type: ContractType,
    /// 月額の契約プラン（ワンショットは `None`）
    pub plan: Option<SubscriptionPlan>,
}
