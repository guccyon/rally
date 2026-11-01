use std::{collections::BTreeMap, time::SystemTime};

use domain::{
    identity::AccountId,
    messaging::{MessageThread, UnansweredThreadCriteria},
    profile::DisplayName,
};

/// 本部が見る、返信のないまま時間の過ぎたスレッドの一覧（[`super::ListUnansweredMessageThreads`] の結果）。
///
/// スレッドはメッセージを含むが、本部の一覧は件数と時間だけを見る（本文を外に出すかは境界が決め、admin-web の契約は
/// 本文を返さない）。
#[derive(Debug, Clone, PartialEq)]
pub struct UnansweredThreadList {
    /// 使った基準（生徒の最後の発言からの時間。省いたときは既定の時間）
    pub criteria: UnansweredThreadCriteria,
    /// 照会した時刻（経過時間はこの時刻までで数える。[`UnansweredThreadCriteria::elapsed`]）
    pub listed_at: SystemTime,
    /// 返信のないまま時間の過ぎたスレッド（選び方と並べ方は domain の [`UnansweredThreadCriteria`]）
    pub threads: Vec<MessageThread>,
    /// `threads` の担当コーチの表示名（コーチの ID ごと）。引けないコーチは含まない
    pub coach_display_names: BTreeMap<AccountId, DisplayName>,
    /// `threads` の生徒の表示名（生徒の ID ごと）。引けない生徒は含まない
    pub student_display_names: BTreeMap<AccountId, DisplayName>,
}
