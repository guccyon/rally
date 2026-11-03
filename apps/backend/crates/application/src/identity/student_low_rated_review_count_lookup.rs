use std::{future::Future, time::SystemTime};

use domain::{identity::AccountId, review::StudentLowRatedReviewCount};

use crate::Unavailable;

/// identity が review に生徒 1 人がつけた直近の低い評価と、そのうち本部が未対応の件数を問い合わせるポート（本部が見る
/// 生徒 1 人の利用の様子に添える）。
///
/// 評価は review の持ち物で、数え方は domain の [`StudentLowRatedReviewCount`]（しきい値と期間は
/// [`domain::review::LowRatedReviewCriteria`]）、identity は読むだけ。実装は
/// [`crate::review::ReviewStudentLowRatedReviewCountLookup`]。
pub trait StudentLowRatedReviewCountLookup: Send + Sync {
    /// 生徒 `student_id` がつけた評価の、`now` に照会したときの直近の低い評価の件数。評価がなければ 0 件。
    fn find_low_rated_review_count(
        &self,
        student_id: AccountId,
        now: SystemTime,
    ) -> impl Future<Output = Result<StudentLowRatedReviewCount, Unavailable>> + Send;
}
