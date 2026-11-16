use std::collections::BTreeMap;

use domain::{identity::AccountId, messaging::MessageThread, profile::DisplayName};

/// アカウントが当事者のスレッドの一覧（[`super::ListMessageThreads`] の結果）。
#[derive(Debug, Clone, PartialEq)]
pub struct MessageThreadList {
    /// 当事者のスレッド（並べ方は [`super::ListMessageThreads`]）
    pub threads: Vec<MessageThread>,
    /// 呼んだアカウントが生徒のスレッドの担当コーチの表示名（コーチの ID ごと）。引けないコーチは含まない。
    /// 呼んだアカウントが担当コーチのスレッドのコーチ（自分）は引かない
    pub coach_display_names: BTreeMap<AccountId, DisplayName>,
}
