use std::collections::BTreeMap;

use domain::{identity::AccountId, profile::DisplayName, scheduling::NextAction};

/// 生徒の次の操作（[`super::GetMyNextActions`] の結果）。
#[derive(Debug, Clone, PartialEq)]
pub struct MyNextActions {
    /// 次の操作。並びがそのまま優先の順で、[`NextAction::LIMIT`] 件まで（決め方は [`NextAction::decide`]）
    pub actions: Vec<NextAction>,
    /// `actions` のコーチの表示名（コーチの ID ごと）。表示名が引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, DisplayName>,
}
