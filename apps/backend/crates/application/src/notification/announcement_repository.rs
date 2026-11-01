use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::notification::{Announcement, AnnouncementId};

use crate::Unavailable;

/// 本部からのお知らせ（[`Announcement`]。送った記録）の保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryAnnouncementRepository`] と、infra の Postgres（`notification.announcements`）。
///
/// 送った記録は変えない（予約送信・取り消しはない）。
pub trait AnnouncementRepository: Send + Sync {
    /// 新しいお知らせの ID。
    fn next_id(&self) -> AnnouncementId;

    /// 送った記録 `announcement` を残す。
    fn insert(
        &self,
        announcement: Announcement,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// 送った記録すべて。送った時刻の新しい順（同じ時刻は後に残したものが先）。
    fn find_all(&self) -> impl Future<Output = Result<Vec<Announcement>, Unavailable>> + Send;

    /// ID が `ids` のどれかの送った記録（ID ごと）。ない ID は結果に入れない。
    fn find_by_ids(
        &self,
        ids: &BTreeSet<AnnouncementId>,
    ) -> impl Future<Output = Result<BTreeMap<AnnouncementId, Announcement>, Unavailable>> + Send;
}
