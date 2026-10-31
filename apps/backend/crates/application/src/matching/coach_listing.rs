use domain::{
    catalog::Sport, identity::AccountId, profile::DisplayName, review::CoachReviewSummary,
};

/// コーチを探す一覧の 1 件（student-app の `CoachListing`）。表示名は profile、評価の件数と平均は review、
/// 公開中のレッスンメニューの数は catalog の持ち物の写し。お気に入りかは matching の持ち物。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachListing {
    pub coach_id: AccountId,
    pub display_name: DisplayName,
    /// 探したスポーツ（コーチが提供するスポーツのうちの 1 つ）
    pub sport: Sport,
    pub review_summary: CoachReviewSummary,
    /// `sport` の公開中のレッスンメニューの数
    pub published_lesson_menu_count: u32,
    /// 探した生徒のお気に入りに入っているか
    pub is_favorite: bool,
}
