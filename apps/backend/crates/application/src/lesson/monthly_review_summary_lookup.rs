use std::{collections::BTreeMap, future::Future};

use domain::{entitlement::YearMonth, identity::AccountId, review::CoachReviewSummary};

use crate::Unavailable;

/// コーチの月ごとのレッスンの実績（[`super::GetMyLessonStats`]）が、review に暦月ごとの評価の件数と平均を問い合わせるポート。
///
/// 評価の持ち主と数え方は review（domain の [`CoachReviewSummary::tally`]）で、ここは読むだけ。評価を暦月に振り分けるのは
/// 評価を送った時刻（日本時間の暦月。`entitlement::calendar_month`）。実装は
/// [`crate::review::ReviewMonthlyReviewSummaryLookup`]。
pub trait MonthlyReviewSummaryLookup: Send + Sync {
    /// コーチ `coach_id` が `months` の各暦月に受けた評価の件数と平均。評価のない月も 0 件として含める。
    fn find_monthly_review_summaries(
        &self,
        coach_id: AccountId,
        months: &[YearMonth],
    ) -> impl Future<Output = Result<BTreeMap<YearMonth, CoachReviewSummary>, Unavailable>> + Send;
}
