use std::future::Future;

use domain::lesson::{LessonCompletion, LessonCompletionId};

use crate::Unavailable;

/// review が完了したレッスンを問い合わせるポート（評価できるのは完了したレッスンだけ）。
///
/// 完了したレッスンの持ち主は live-lesson / async-lesson（docs/adr/0006）で、review は読むだけ。評価できるかの
/// 判定は domain の [`domain::review::LessonReview::submit`] が行う。実装は
/// [`crate::lesson::LessonCompletionLookup`]。
pub trait CompletedLessonLookup: Send + Sync {
    /// `lesson_id` の完了したレッスン。なければ `None`。
    fn find_completion(
        &self,
        lesson_id: LessonCompletionId,
    ) -> impl Future<Output = Result<Option<LessonCompletion>, Unavailable>> + Send;
}
