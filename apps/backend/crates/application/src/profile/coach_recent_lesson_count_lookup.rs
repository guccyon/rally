use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, lesson::CoachLessonCount};

use crate::Unavailable;

/// profile がコーチごとの直近に終わったレッスンの数を問い合わせるポート（本部が見るコーチの一覧に、1 件ごとに添える）。
///
/// 完了したレッスンの記録は live-lesson と async-lesson が発行し（docs/adr/0006）、数え方と期間は domain の
/// [`CoachLessonCount::tally_recent`]（[`domain::lesson::RecentLessonPeriod`]）で、profile は読むだけ。実装は
/// [`crate::lesson::LessonCoachRecentLessonCountLookup`]。
pub trait CoachRecentLessonCountLookup: Send + Sync {
    /// `coach_ids` のコーチの、照会した時刻の直近の期間に完了したレッスンの数。完了したレッスンのないコーチも 0 件として
    /// 含める。
    fn find_recent_lesson_counts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, CoachLessonCount>, Unavailable>> + Send;
}
