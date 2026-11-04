use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, review::CoachLowRatedReviewCount};

use crate::Unavailable;

/// profile がコーチごとの直近の低い評価と、そのうち本部が未対応の件数を問い合わせるポート（本部が見るコーチの一覧を
/// 未対応の低い評価のあるコーチに絞るのに使う）。
///
/// 評価は review の持ち物で、数え方は domain の [`CoachLowRatedReviewCount::tally`]（しきい値と期間は
/// [`domain::review::LowRatedReviewCriteria`]）で、profile は読むだけ。実装は
/// [`crate::review::ReviewCoachLowRatedReviewCountLookup`]。
pub trait CoachLowRatedReviewCountLookup: Send + Sync {
    /// `coach_ids` のコーチの、照会した時刻で数えた直近の低い評価の件数。評価のないコーチも 0 件として含める。
    fn find_low_rated_review_counts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, CoachLowRatedReviewCount>, Unavailable>> + Send;
}
