use std::{future::Future, time::SystemTime};

use domain::{
    DomainError,
    identity::AccountId,
    lesson::{LessonCompletion, LessonCompletionId},
};

use super::RecordCompletionError;
use crate::Unavailable;

/// レッスンの完了記録の保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryLessonCompletionRepository`]。
///
/// Postgres では完了記録を元の集約のコンテキストのスキーマに置く（同期は `live_lesson.lesson_completions`、
/// 非同期は `async_lesson.lesson_completions`。docs/adr/0006）。実装は通話セッション・動画の保存先を Postgres に
/// するときに足し、生徒ごとの advisory lock で 2 つのテーブルにまたがる記録を直列にする。
///
/// 不変条件の判定（`is_first_lesson`・同じレッスンの重複）はクロージャ（domain の集約の `complete`）だけが行い、
/// 保存先は判定しない。その代わり、同じ生徒の読み取りから保存までを直列にする（並行な記録が互いを見ずに
/// 両方とも初回になることがないように）。
pub trait LessonCompletionRepository: Send + Sync {
    /// 新しい完了記録の ID を払い出す。
    fn next_id(&self) -> LessonCompletionId;

    /// `lesson_id` の完了記録（同期・非同期のどちらでも）。なければ `None`。
    fn find(
        &self,
        lesson_id: LessonCompletionId,
    ) -> impl Future<Output = Result<Option<LessonCompletion>, Unavailable>> + Send;

    /// コーチ `coach_id` の完了記録（同期・非同期の両方）のうち、完了した時刻が `completed_since` 以後のものをすべて読む
    /// （数えない・並べない。数えるのは domain の [`domain::lesson::CoachLessonCount::tally`]）。
    fn find_by_coach(
        &self,
        coach_id: AccountId,
        completed_since: SystemTime,
    ) -> impl Future<Output = Result<Vec<LessonCompletion>, Unavailable>> + Send;

    /// `student_id` の生徒の完了記録（同期・非同期の両方）をすべて読み、`record` に渡す。`record` が `Ok` なら
    /// 返した記録を保存して返す。
    fn record<F>(
        &self,
        student_id: AccountId,
        record: F,
    ) -> impl Future<Output = Result<LessonCompletion, RecordCompletionError>> + Send
    where
        F: FnOnce(&[LessonCompletion]) -> Result<LessonCompletion, DomainError> + Send;
}
