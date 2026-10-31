use std::future::Future;

use domain::{catalog::Sport, lesson::LessonCompletionId};

use crate::Unavailable;

/// review が、評価した完了したレッスンのスポーツを問い合わせるポート（本部が見る低い評価の一覧をスポーツで絞る）。
///
/// スポーツの持ち主は完了したレッスンの元（同期レッスンは scheduling の枠、非同期レッスンは async-lesson の動画）で、
/// review は読むだけ（枠・動画の中身を直接読まない）。実装は [`crate::lesson::LessonCompletionSportLookup`]。
pub trait ReviewedLessonSportLookup: Send + Sync {
    /// `lesson_id` の完了したレッスンのスポーツ（元のスポーツ）。完了したレッスン・元がなければ `None`。
    fn find_lesson_sport(
        &self,
        lesson_id: LessonCompletionId,
    ) -> impl Future<Output = Result<Option<Sport>, Unavailable>> + Send;
}
