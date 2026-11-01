use std::{collections::BTreeMap, future::Future, time::SystemTime};

use domain::{
    identity::AccountId,
    notification::{AnnouncementId, Notification, NotificationId},
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

    /// 本部からのお知らせごとの、生徒ごとのお知らせのうち既読のもの（[`Notification::is_read`]）の数。`read_until` の
    /// キーが数えるお知らせで、値が数えに入れる既読の時刻の上限（その時刻ちょうどまでを含む。`None` なら上限なし。決めるのは
    /// [`domain::notification::Announcement::reads_counted_until`]）。数えるものがない ID は結果に入れない（0 件）。
    fn count_read_by_announcement(
        &self,
        read_until: &BTreeMap<AnnouncementId, Option<SystemTime>>,
    ) -> impl Future<Output = Result<BTreeMap<AnnouncementId, u32>, Unavailable>> + Send;
}
