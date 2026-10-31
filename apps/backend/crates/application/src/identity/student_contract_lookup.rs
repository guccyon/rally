use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::identity::AccountId;

use super::StudentContract;
use crate::Unavailable;

/// identity が entitlement に生徒の受講権の契約種別と契約プランを問い合わせるポート（本部が見る生徒の一覧に添える）。
///
/// 受講権の持ち主は entitlement で、identity は読むだけ。実装は
/// [`crate::entitlement::EntitlementStudentContractLookup`]。
pub trait StudentContractLookup: Send + Sync {
    /// `student_ids` の生徒の受講権の契約種別と契約プラン。受講権がない生徒は結果に含めない。
    fn find_student_contracts(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, StudentContract>, Unavailable>> + Send;
}
