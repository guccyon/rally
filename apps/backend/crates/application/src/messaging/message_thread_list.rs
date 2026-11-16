use std::collections::BTreeMap;

use domain::{identity::AccountId, messaging::MessageThread, profile::DisplayName};

/// 呼んだアカウントが当事者のスレッドの一覧（[`super::ListMessageThreads`] の結果）。
#[derive(Debug, Clone, PartialEq)]
pub struct MessageThreadList {
    /// スレッド（並べ方は [`super::ListMessageThreads`]）
    pub threads: Vec<MessageThread>,
    /// 呼んだアカウントが担当コーチのスレッドの生徒の表示名（生徒の ID ごと）。引けない生徒と、生徒が呼んだときは含まない
    pub student_display_names: BTreeMap<AccountId, DisplayName>,
}
