use std::collections::BTreeMap;

use domain::{identity::AccountId, live_lesson::LiveLessonSession, profile::DisplayName};
use time::Date;

/// 本部が期間とコーチ・生徒で見る通話セッションの記録の一覧（[`super::ListLiveLessonSessions`] の結果）。
#[derive(Debug, Clone, PartialEq)]
pub struct LiveLessonSessionHistory {
    /// 期間の始まりの日付（日本時間。含む）
    pub from: Date,
    /// 期間の終わりの日付（日本時間。含む）
    pub to: Date,
    /// 期間に開始したセッション（選び方と並べ方は domain の [`domain::live_lesson::SessionHistoryCriteria`]）
    pub sessions: Vec<LiveLessonSession>,
    /// `sessions` のコーチの表示名（コーチの ID ごと）。引けないコーチは含まない
    pub coach_display_names: BTreeMap<AccountId, DisplayName>,
    /// `sessions` を開始した予約の生徒の表示名（生徒の ID ごと）。引けない生徒は含まない
    pub student_display_names: BTreeMap<AccountId, DisplayName>,
}
