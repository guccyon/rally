//! entitlement の値と、DB に保存する文字列の対応表。
//!
//! `entitlement.entitlements.contract_type`・`entitlement.subscriptions.status`（CHECK 制約の値）はこの表だけを通す。
//! `sport` は [`crate::identity_values::sport_value`] と同じ値。CHECK との一致は `tests/schema.rs` が確かめる。

use domain::entitlement::{ContractType, SubscriptionStatus};

/// 対応表の全ての契約種別（DB の CHECK の並び順）。
pub const CONTRACT_TYPES: [ContractType; 2] = [ContractType::Monthly, ContractType::OneShot];

/// 対応表の全ての月額契約の状態（DB の CHECK の並び順）。
pub const SUBSCRIPTION_STATUSES: [SubscriptionStatus; 2] =
    [SubscriptionStatus::Active, SubscriptionStatus::Cancelled];

/// `entitlement.entitlements.contract_type` の値（月額は契約の `ContractType` と同じ `subscription`）。
pub const fn contract_type_value(contract_type: ContractType) -> &'static str {
    match contract_type {
        ContractType::Monthly => "subscription",
        ContractType::OneShot => "one_shot",
    }
}

pub fn contract_type_from_value(value: &str) -> Option<ContractType> {
    CONTRACT_TYPES
        .into_iter()
        .find(|contract_type| contract_type_value(*contract_type) == value)
}

/// `entitlement.subscriptions.status` の値。
pub const fn subscription_status_value(status: SubscriptionStatus) -> &'static str {
    match status {
        SubscriptionStatus::Active => "active",
        SubscriptionStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip() {
        for contract_type in CONTRACT_TYPES {
            assert_eq!(
                contract_type_from_value(contract_type_value(contract_type)),
                Some(contract_type)
            );
        }
    }

    #[test]
    fn unknown_values_are_none() {
        for value in ["", "monthly", "Subscription", " one_shot"] {
            assert_eq!(contract_type_from_value(value), None, "{value:?}");
        }
    }
}
