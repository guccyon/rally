use std::future::Future;

use domain::{lesson::LessonId, review::ReviewLessonMenu};

use crate::Unavailable;

/// review が、評価した完了したレッスンのレッスンメニューを問い合わせるポート（コーチが受けた評価の一覧に添える）。
///
/// メニューの写しの持ち主は完了したレッスンの元（同期レッスンは scheduling の予約、非同期レッスンは async-lesson の
/// 動画）で、review は読むだけ（予約・動画の中身を直接読まない）。実装は
/// [`crate::lesson::LessonCompletionMenuLookup`]。
pub trait ReviewedLessonMenuLookup: Send + Sync {
    /// `lesson_id` の完了したレッスンの元が持つメニューの写し。完了したレッスン・元がない、または元がメニューを
    /// 持たなければ `None`。
    fn find_lesson_menu(
        &self,
        lesson_id: LessonId,
    ) -> impl Future<Output = Result<Option<ReviewLessonMenu>, Unavailable>> + Send;
}
