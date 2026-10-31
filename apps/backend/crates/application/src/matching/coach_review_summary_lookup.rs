use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, review::CoachReviewSummary};

use crate::Unavailable;

/// matching が review にコーチごとの評価の件数と平均を問い合わせるポート。
///
/// 評価の持ち主と数え方は review（domain の [`CoachReviewSummary::tally`]）で、matching は読むだけ。実装は
/// [`crate::review::ReviewCoachReviewSummaryLookup`]。
pub trait CoachReviewSummaryLookup: Send + Sync {
    /// `coach_ids` のコーチの評価の件数と平均。評価のないコーチも 0 件として含める。
    fn find_review_summaries(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, CoachReviewSummary>, Unavailable>> + Send;
}
