use std::{future::Future, time::SystemTime};

use domain::{
    identity::AccountId,
    notification::{Notification, NotificationId},
};

use crate::Unavailable;

/// お知らせ（[`Notification`]）の保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryNotificationRepository`] と、infra の Postgres（`notification.notifications`）。
///
/// 同じプッシュ通知（[`Notification::push`] が等しい）のお知らせは 1 件だけ残す（境界間イベントの再送や、プッシュ通知を
/// 送れずにやり直したときに 2 件にしない）。
pub trait NotificationRepository: Send + Sync {
    /// 新しいお知らせの ID。
    fn next_id(&self) -> NotificationId;

    /// お知らせ `notification` を残す。同じプッシュ通知のお知らせがすでにあれば何もしない（障害にもしない）。
    fn insert(
        &self,
        notification: Notification,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// `recipient` が受け取るお知らせ。作った時刻の新しい順（同じ時刻は後に残したものが先）。
    fn find_by_recipient(
        &self,
        recipient: AccountId,
    ) -> impl Future<Output = Result<Vec<Notification>, Unavailable>> + Send;

    /// `recipient` が受け取るお知らせを、時刻 `at` に読んだことにする（[`Notification::mark_read`]。読んだものは最初に
    /// 読んだ時刻のまま、`at` より後に作ったものは読んでいないまま）。
    fn mark_all_read(
        &self,
        recipient: AccountId,
        at: SystemTime,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;
}
