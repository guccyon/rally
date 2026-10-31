use std::{collections::BTreeSet, time::SystemTime};

use domain::{
    catalog::Sport, identity::AccountId, profile::DisplayName, review::CoachReviewSummary,
};

/// お気に入りのコーチの一覧の 1 件（student-app の `FavoriteCoach`）。お気に入りは matching の持ち物で、表示名と
/// 提供スポーツは profile、評価の件数と平均は review の持ち物の写し。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FavoriteCoachListing {
    pub coach_id: AccountId,
    pub display_name: DisplayName,
    /// コーチの提供スポーツ（どのクライアントでどのスポーツを見せるかは境界が決める。ゴルフも含む）
    pub sports: BTreeSet<Sport>,
    pub review_summary: CoachReviewSummary,
    /// お気に入りに入れた時刻
    pub favorited_at: SystemTime,
}
