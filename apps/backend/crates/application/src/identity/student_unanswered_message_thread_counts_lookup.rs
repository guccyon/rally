use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    time::SystemTime,
};

use domain::{identity::AccountId, messaging::StudentUnansweredMessageThreadCount};

use crate::Unavailable;

/// identity が messaging に生徒ごとの返信のないまま時間の過ぎたスレッドの数をまとめて問い合わせるポート（本部が見る生徒の
/// 一覧を返事を待っているメッセージのある生徒に絞るのに使う）。
///
/// スレッドの持ち主は messaging で、数え方は生徒 1 人の利用の様子（[`super::StudentUnansweredMessageThreadCountLookup`]）と
/// 同じ domain の [`StudentUnansweredMessageThreadCount`]（基準は [`domain::messaging::UnansweredThreadCriteria`] の既定）、
/// identity は読むだけ。実装は [`crate::messaging::MessagingStudentUnansweredMessageThreadCountLookup`]。
pub trait StudentUnansweredMessageThreadCountsLookup: Send + Sync {
    /// `student_ids` の生徒の、`now` に照会したときに返信のないまま既定の時間の過ぎたスレッドの数。スレッドのない生徒も
    /// 0 件として含める。
    fn find_unanswered_message_thread_counts(
        &self,
        student_ids: &BTreeSet<AccountId>,
        now: SystemTime,
    ) -> impl Future<
        Output = Result<BTreeMap<AccountId, StudentUnansweredMessageThreadCount>, Unavailable>,
    > + Send;
}
