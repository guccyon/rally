use std::future::Future;

use domain::notification::{AnnouncementDraft, AnnouncementDraftId};

use crate::Unavailable;

/// 本部からのお知らせの下書き（[`AnnouncementDraft`]）の保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryAnnouncementDraftRepository`] と、infra の Postgres（`notification.announcement_drafts`）。
///
/// 下書きは本部が直す・消す・送る（送ったら消す）。送った記録（[`super::AnnouncementRepository`]）とは別に持つ。
pub trait AnnouncementDraftRepository: Send + Sync {
    /// 新しい下書きの ID。
    fn next_id(&self) -> AnnouncementDraftId;

    /// 下書き `draft` を残す。
    fn insert(
        &self,
        draft: AnnouncementDraft,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// ID が `id` の下書き。なければ `None`。
    fn find(
        &self,
        id: AnnouncementDraftId,
    ) -> impl Future<Output = Result<Option<AnnouncementDraft>, Unavailable>> + Send;

    /// 既存の下書きの題・本文・対象のスポーツ・最後に直した時刻を `draft` のもので置き換える（ID で探す。作った本部の
    /// アカウントと作った時刻は変えない）。下書きがなければ（消されていれば）何もせず `false`。
    fn update(
        &self,
        draft: AnnouncementDraft,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;

    /// ID が `id` の下書きを消す。なければ何もせず `false`。
    fn delete(
        &self,
        id: AnnouncementDraftId,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;

    /// 下書きすべて。最後に直した時刻の新しい順（同じ時刻は後に作ったものが先）。
    fn find_all(&self) -> impl Future<Output = Result<Vec<AnnouncementDraft>, Unavailable>> + Send;
}
