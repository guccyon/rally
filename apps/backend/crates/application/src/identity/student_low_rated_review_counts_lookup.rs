use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    time::SystemTime,
};

use domain::{identity::AccountId, review::StudentLowRatedReviewCount};

use crate::Unavailable;

/// identity が review に生徒ごとの直近の低い評価と、そのうち本部が未対応の件数をまとめて問い合わせるポート（本部が見る
/// 生徒の一覧を未対応の低い評価をつけた生徒に絞るのに使う）。
///
/// 評価は review の持ち物で、数え方は生徒 1 人の利用の様子（[`super::StudentLowRatedReviewCountLookup`]）と同じ domain の
/// [`StudentLowRatedReviewCount::tally`]（しきい値と期間は [`domain::review::LowRatedReviewCriteria`]）、identity は読むだけ。
/// 実装は [`crate::review::ReviewStudentLowRatedReviewCountLookup`]。
pub trait StudentLowRatedReviewCountsLookup: Send + Sync {
    /// `student_ids` の生徒の、`now` に照会したときの直近の低い評価の件数。評価のない生徒も 0 件として含める。
    fn find_low_rated_review_counts(
        &self,
        student_ids: &BTreeSet<AccountId>,
        now: SystemTime,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, StudentLowRatedReviewCount>, Unavailable>> + Send;
}
