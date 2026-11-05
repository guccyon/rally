use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{async_lesson::CoachPendingVideoReviewCount, identity::AccountId};

use crate::Unavailable;

/// profile が async-lesson にコーチごとの返却待ちの動画添削の件数（と、そのうち返却の遅れている件数）をまとめて問い合わせる
/// ポート（本部が見るコーチの一覧を返却の遅れている動画のあるコーチに絞るのに使う）。
///
/// 動画の持ち主は async-lesson で、数え方はコーチ 1 人の様子（`getCoachOverview` の
/// [`crate::async_lesson::GetCoachPendingVideoReviewCount`]）と同じ domain の [`CoachPendingVideoReviewCount::tally`]、
/// profile は読むだけ。実装は [`crate::async_lesson::AsyncLessonCoachPendingVideoReviewCountsLookup`]。
pub trait CoachPendingVideoReviewCountsLookup: Send + Sync {
    /// `coach_ids` のコーチの、照会した時刻に返却を待っている動画添削の件数。動画のないコーチも 0 件として含める。
    fn find_pending_video_review_counts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, CoachPendingVideoReviewCount>, Unavailable>>
    + Send;
}
