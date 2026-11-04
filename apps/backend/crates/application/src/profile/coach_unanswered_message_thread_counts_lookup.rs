use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{identity::AccountId, messaging::CoachUnansweredMessageThreadCount};

use crate::Unavailable;

/// profile が messaging にコーチごとの返信のないまま時間の過ぎたスレッドの数をまとめて問い合わせるポート（本部が見るコーチの
/// 一覧を返事のないメッセージのあるコーチに絞るのに使う）。
///
/// スレッドの持ち主は messaging で、数え方はコーチ 1 人の様子（[`super::CoachUnansweredMessageThreadCountLookup`]）と
/// 同じ domain の [`CoachUnansweredMessageThreadCount`]（基準は [`domain::messaging::UnansweredThreadCriteria`] の既定）、
/// profile は読むだけ。実装は [`crate::messaging::MessagingCoachUnansweredMessageThreadCountLookup`]。
pub trait CoachUnansweredMessageThreadCountsLookup: Send + Sync {
    /// `coach_ids` のコーチの、照会した時刻に返信のないまま既定の時間の過ぎたスレッドの数。スレッドのないコーチも 0 件として
    /// 含める。
    fn find_unanswered_message_thread_counts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<
        Output = Result<BTreeMap<AccountId, CoachUnansweredMessageThreadCount>, Unavailable>,
    > + Send;
}
