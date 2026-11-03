use domain::review::Rating;
use time::Date;

/// コーチが受けた評価の一覧（[`super::ListCoachReviews`]）の条件。どれも省ける（すべて省けば今までどおりすべて）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListCoachReviewsQuery {
    /// この点以下（含む）の評価だけ
    pub max_rating: Option<Rating>,
    /// 評価の付いた期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 評価の付いた期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
}
