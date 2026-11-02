use domain::{
    async_lesson::{CoachPendingVideoReviewCount, VideoReviewLeadTime},
    profile::DisplayName,
    review::{CoachLowRatedReviewCount, CoachReviewSummary},
    scheduling::{CoachAcceptanceStatus, CoachRecentCancelledBookingCount},
};

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
    /// 直近の低い評価の件数と、そのうち本部が未対応の件数（review の `GetCoachLowRatedReviewCount`）
    pub low_rated_reviews: CoachLowRatedReviewCount,
    /// 今月と先月のレッスンの実績（lesson の `GetMyLessonStats`）
    pub lesson_stats: CoachLessonStats,
    /// 公開中のレッスンメニューの数（catalog の `ListCoachLessonMenus`。スポーツで絞らない）
    pub published_lesson_menu_count: u32,
    /// 直近 30 日に送られた動画の返却までの時間の中央値と、そのうち返した動画添削の件数（async-lesson の
    /// `GetCoachVideoReviewLeadTime`）
    pub video_review_lead_time: VideoReviewLeadTime,
    /// 返却待ちの動画添削の件数と、そのうち送付から 48 時間以上たった件数（async-lesson の
    /// `GetCoachPendingVideoReviewCount`）
    pub pending_video_reviews: CoachPendingVideoReviewCount,
    /// 直近 30 日にキャンセルされた予約の数（scheduling の `CoachRecentCancelledBookingCountLookup`。誰がキャンセルしたかを
    /// 分けない）
    pub recent_cancelled_bookings: CoachRecentCancelledBookingCount,
}
