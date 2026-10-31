use std::future::Future;

use domain::{identity::AccountId, lesson::LessonCompletion};

use crate::Unavailable;

/// identity が完了記録（[`crate::lesson`]）に生徒 1 人の直近の受けたレッスンを問い合わせるポート（本部が見る生徒 1 人の
/// 利用の様子に添える）。
///
/// 完了記録の持ち主は live-lesson・async-lesson（保存は [`crate::lesson`]）で、identity は読むだけ。実装は
/// [`crate::lesson::LessonStudentLessonHistoryLookup`]。
pub trait StudentLessonHistoryLookup: Send + Sync {
    /// 生徒 `student_id` の完了したレッスン（同期・非同期の両方）を、完了した時刻の新しい順（同じ時刻は完了記録の ID の
    /// 順）に最大 `limit` 件。
    fn find_recent_lessons(
        &self,
        student_id: AccountId,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<LessonCompletion>, Unavailable>> + Send;
}
