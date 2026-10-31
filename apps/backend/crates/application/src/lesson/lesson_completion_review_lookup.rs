use std::{collections::BTreeSet, future::Future};

use domain::{identity::AccountId, lesson::LessonCompletionId};

use crate::Unavailable;

/// 完了記録が review に「生徒がその完了したレッスンに評価を付けたか」を問い合わせるポート（生徒が受けたレッスンの
/// 一覧で、評価済みの印を添える）。
///
/// 評価の持ち主は review で、完了記録は読むだけ。実装は [`crate::review::ReviewLessonCompletionReviewLookup`]。
pub trait LessonCompletionReviewLookup: Send + Sync {
    /// `lesson_ids` のうち、生徒 `student_id` が評価を付けた完了したレッスン。
    fn find_reviewed_lessons(
        &self,
        student_id: AccountId,
        lesson_ids: &BTreeSet<LessonCompletionId>,
    ) -> impl Future<Output = Result<BTreeSet<LessonCompletionId>, Unavailable>> + Send;
}
