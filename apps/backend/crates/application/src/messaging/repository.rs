use std::future::Future;

use domain::{
    identity::AccountId,
    messaging::{MessageId, MessageThread, MessageThreadId, MessagingError},
};

use super::UpdateMessageThreadError;
use crate::Unavailable;

/// スレッドとメッセージの保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryMessageThreadRepository`] と、infra の Postgres（`messaging.threads` / `messages`）。
///
/// 同じ生徒とコーチのスレッドは 1 つ（[`Self::open`]。DB では `threads_student_id_coach_id_key`）。不変条件の判定は
/// domain の [`MessageThread`] が行い、保存先は読み取りから保存までをスレッドごとに直列にする（並行な送信が
/// 互いの順序を見落とさないように）。
pub trait MessageThreadRepository: Send + Sync {
    /// 新しいスレッドの ID を払い出す。
    fn next_thread_id(&self) -> MessageThreadId;

    /// 新しいメッセージの ID を払い出す。
    fn next_message_id(&self) -> MessageId;

    /// 開いたスレッドを保存し、保存したスレッドを返す。同じ生徒とコーチのスレッドがすでにあれば何も保存せず、
    /// 既存のスレッドを返す（同じ予約の確定が 2 回届いても、別の予約で同じコーチと確定しても 1 つ）。
    fn open(
        &self,
        thread: MessageThread,
    ) -> impl Future<Output = Result<MessageThread, Unavailable>> + Send;

    /// `thread_id` のスレッド（メッセージを含む）。なければ `None`。
    fn find(
        &self,
        thread_id: MessageThreadId,
    ) -> impl Future<Output = Result<Option<MessageThread>, Unavailable>> + Send;

    /// `account_id` が当事者（生徒か担当コーチ）のスレッドすべて（メッセージを含む。順序は問わない）。
    fn list_for(
        &self,
        account_id: AccountId,
    ) -> impl Future<Output = Result<Vec<MessageThread>, Unavailable>> + Send;

    /// `thread_id` のスレッドを読み、`update` に渡す。`update` が `Ok` なら保存して返す（`Err` なら何も保存しない）。
    fn update<F>(
        &self,
        thread_id: MessageThreadId,
        update: F,
    ) -> impl Future<Output = Result<MessageThread, UpdateMessageThreadError>> + Send
    where
        F: FnOnce(&mut MessageThread) -> Result<(), MessagingError> + Send;
}
