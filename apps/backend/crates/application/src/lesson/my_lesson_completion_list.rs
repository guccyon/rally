use std::collections::BTreeMap;

use domain::{identity::AccountId, profile::DisplayName};

use super::MyLessonCompletion;

/// 生徒が受けたレッスンの一覧（[`super::ListMyLessonCompletions`] の結果）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MyLessonCompletionList {
    /// 生徒の完了したレッスン（同期・非同期の両方）。完了した時刻の新しい順で、同じ時刻は完了記録の ID の順
    pub completions: Vec<MyLessonCompletion>,
    /// `completions` のコーチの表示名（コーチの ID ごと）。表示名が引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, DisplayName>,
}
