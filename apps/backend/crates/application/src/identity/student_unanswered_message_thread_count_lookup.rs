use std::{future::Future, time::SystemTime};

use domain::{identity::AccountId, messaging::StudentUnansweredMessageThreadCount};

use crate::Unavailable;

/// identity が messaging に生徒 1 人の返信のないまま時間の過ぎたスレッドの数を問い合わせるポート（本部が見る生徒 1 人の
/// 利用の様子に添える）。
///
/// スレッドの持ち主は messaging で、数え方は domain の [`StudentUnansweredMessageThreadCount`]（基準は
/// [`domain::messaging::UnansweredThreadCriteria`] の既定）、identity は読むだけ。実装は
/// [`crate::messaging::MessagingStudentUnansweredMessageThreadCountLookup`]。
pub trait StudentUnansweredMessageThreadCountLookup: Send + Sync {
    /// 生徒 `student_id` の、`now` に照会したときに返信のないまま既定の時間の過ぎたスレッドの数。スレッドがなければ 0 件。
    fn find_unanswered_message_thread_count(
        &self,
        student_id: AccountId,
        now: SystemTime,
    ) -> impl Future<Output = Result<StudentUnansweredMessageThreadCount, Unavailable>> + Send;
}
