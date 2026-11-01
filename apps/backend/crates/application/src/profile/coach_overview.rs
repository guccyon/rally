use domain::{profile::DisplayName, review::CoachReviewSummary, scheduling::CoachAcceptanceStatus};

use crate::lesson::CoachLessonStats;

/// 本部が見るコーチ 1 人の様子（[`super::GetCoachOverview`] の結果）。各項目は持ち主のコンテキストのユースケース・ポートが
/// 返したものをそのまま持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachOverview {
    pub display_name: DisplayName,
    /// 受付の状態（scheduling の持ち物）
    pub acceptance: CoachAcceptanceStatus,
    /// これまでのすべての評価の件数と平均（review の `GetCoachReviewSummary`）
    pub reviews: CoachReviewSummary,
    /// 今月と先月のレッスンの実績（lesson の `GetMyLessonStats`）
    pub lesson_stats: CoachLessonStats,
    /// 公開中のレッスンメニューの数（catalog の `ListCoachLessonMenus`。スポーツで絞らない）
    pub published_lesson_menu_count: u32,
}
