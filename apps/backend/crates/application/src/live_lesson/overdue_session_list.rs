use std::{collections::BTreeMap, time::SystemTime};

use domain::{
    identity::AccountId,
    live_lesson::{LiveLessonSession, OverdueSessionCriteria},
    profile::DisplayName,
};

/// 本部が見る、終わっていないまま時間の過ぎた通話セッションの一覧（[`super::ListOverdueLiveLessonSessions`] の結果）。
#[derive(Debug, Clone, PartialEq)]
pub struct OverdueSessionList {
    /// 使った基準（開始からの時間。省いたときは既定の時間）
    pub criteria: OverdueSessionCriteria,
    /// 照会した時刻（開始からの経過時間はこの時刻までで数える。[`LiveLessonSession::elapsed`]）
    pub listed_at: SystemTime,
    /// 時間の過ぎたセッション（選び方と並べ方は domain の [`OverdueSessionCriteria`]）
    pub sessions: Vec<LiveLessonSession>,
    /// `sessions` のコーチの表示名（コーチの ID ごと）。引けないコーチは含まない
    pub coach_display_names: BTreeMap<AccountId, DisplayName>,
    /// `sessions` を開始した予約の生徒の表示名（生徒の ID ごと）。引けない生徒は含まない
    pub student_display_names: BTreeMap<AccountId, DisplayName>,
}
