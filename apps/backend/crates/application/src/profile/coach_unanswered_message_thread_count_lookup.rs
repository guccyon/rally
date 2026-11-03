use std::future::Future;

use domain::{identity::AccountId, messaging::CoachUnansweredMessageThreadCount};

use crate::Unavailable;

/// profile が messaging にコーチの返信のないまま時間の過ぎたスレッドの数を問い合わせるポート（本部が見るコーチ 1 人の様子に
/// 添える）。
///
/// スレッドの持ち主は messaging で、数え方は domain の [`CoachUnansweredMessageThreadCount`]（基準は
/// [`domain::messaging::UnansweredThreadCriteria`] の既定）、profile は読むだけ。実装は
/// [`crate::messaging::MessagingCoachUnansweredMessageThreadCountLookup`]。
pub trait CoachUnansweredMessageThreadCountLookup: Send + Sync {
    /// `coach_id` のコーチの、照会した時刻に返信のないまま既定の時間の過ぎたスレッドの数。スレッドがなければ 0 件。
    fn find_unanswered_message_thread_count(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<CoachUnansweredMessageThreadCount, Unavailable>> + Send;
}
