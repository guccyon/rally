use std::{future::Future, time::SystemTime};

use domain::identity::AccountId;

use super::StudentVideoActivity;
use crate::Unavailable;

/// identity が async-lesson に生徒 1 人の動画添削の様子（送った動画の数と返却待ちの数）を問い合わせるポート（本部が見る
/// 生徒 1 人の利用の様子に添える）。
///
/// 動画の持ち主は async-lesson で、identity は読むだけ。実装は
/// [`crate::async_lesson::AsyncLessonStudentVideoActivityLookup`]。
pub trait StudentVideoActivityLookup: Send + Sync {
    /// 生徒 `student_id` の、送った動画の数と `now` に返却を待っている動画の数（[`StudentVideoActivity`]）。
    /// 動画がなければどちらも 0。
    fn find_video_activity(
        &self,
        student_id: AccountId,
        now: SystemTime,
    ) -> impl Future<Output = Result<StudentVideoActivity, Unavailable>> + Send;
}
