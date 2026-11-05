use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    time::SystemTime,
};

use domain::identity::AccountId;

use crate::Unavailable;

/// identity が async-lesson に生徒ごとの返却待ちの動画の数をまとめて問い合わせるポート（本部が見る生徒の一覧を
/// 返却待ちの動画のある生徒に絞るのに使う）。
///
/// 動画の持ち主は async-lesson で、数え方は生徒 1 人の利用の様子（[`super::StudentVideoActivityLookup`] の
/// [`super::StudentVideoActivity::pending_review_count`]）と同じ domain の
/// [`domain::async_lesson::PendingVideoReviewFilter`] の既定の条件、identity は読むだけ。実装は
/// [`crate::async_lesson::AsyncLessonStudentVideoActivityLookup`]。
pub trait StudentPendingVideoReviewCountsLookup: Send + Sync {
    /// `student_ids` の生徒の、`now` に照会したときに返却を待っている動画の数。返却待ちの動画のない生徒も 0 件として
    /// 含める。
    fn find_pending_video_review_counts(
        &self,
        student_ids: &BTreeSet<AccountId>,
        now: SystemTime,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, u32>, Unavailable>> + Send;
}
