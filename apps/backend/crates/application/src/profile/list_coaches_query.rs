use domain::{catalog::Sport, profile::DisplayNameQuery, scheduling::CoachAcceptanceStatus};

/// 本部が見るコーチの一覧（[`super::ListCoaches`]）の条件。どれも省け、省いた条件では絞り込まない（すべて省けば
/// すべてのコーチ）。付けた条件はすべてを満たすコーチだけを残す。並べ方は条件で変わらない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListCoachesQuery {
    /// あれば表示名がこれに部分一致するコーチだけ（一致の規則は [`DisplayNameQuery`]）
    pub name: Option<DisplayNameQuery>,
    /// あればこの受付の状態のコーチだけ
    pub acceptance: Option<CoachAcceptanceStatus>,
    /// あればこのスポーツを提供する（提供スポーツのどれかが一致する）コーチだけ
    pub sport: Option<Sport>,
    /// `true` なら直近の期間に完了したレッスンのない（`recent_lesson_count` が 0 の）コーチだけ（判定は
    /// [`domain::lesson::InactiveCoachFilter`]）。`false`（既定）なら絞らない
    pub inactive: bool,
    /// `true` なら直近の低い評価のうち本部が未対応のもの（`getCoachOverview` の `unhandled_low_rated_review_count`）が
    /// 1 件以上あるコーチだけ（判定は [`domain::review::UnhandledLowRatedCoachFilter`]）。`false`（既定）なら絞らない
    pub has_unhandled_low_rated: bool,
    /// `true` なら返信のないまま既定の時間の過ぎたスレッド（`getCoachOverview` の `unanswered_message_thread_count`。
    /// `listUnansweredMessageThreads` に既定の条件とそのコーチの `coach_id` だけを付けたときに載るもの）が 1 件以上ある
    /// コーチだけ（判定は [`domain::messaging::UnansweredCoachFilter`]）。`false`（既定）なら絞らない
    pub has_unanswered: bool,
    /// `true` なら返却待ちのうち送付から 48 時間以上たった動画（`getCoachOverview` の
    /// `overdue_pending_video_review_count`。`listPendingVideoReviews` にそのコーチの `coach_id` と `min_elapsed_hours=48` を
    /// 付けたときに載るもの）が 1 件以上あるコーチだけ（判定は [`domain::async_lesson::OverdueVideoCoachFilter`]）。
    /// `false`（既定）なら絞らない
    pub has_overdue_video: bool,
}
