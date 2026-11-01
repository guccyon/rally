use domain::notification::AnnouncementDraft;

use super::AnnouncementDraftRepository;
use crate::Unavailable;

/// 本部からのお知らせの下書きの一覧のユースケース（admin-web の `listAnnouncementDrafts`。本部だけが呼ぶ）。照会だけで、
/// 何も保存しない。最後に直した時刻の新しい順（同じ時刻は後に作ったものが先）で、どの本部のアカウントが作ったかで
/// 絞らない。送った下書きは消えているので載らない。誰が呼べるか（役割）は境界（interface）が確かめる。行動イベントは
/// 発火しない。
pub struct ListAnnouncementDrafts<D> {
    drafts: D,
}

impl<D> ListAnnouncementDrafts<D>
where
    D: AnnouncementDraftRepository,
{
    pub fn new(drafts: D) -> Self {
        Self { drafts }
    }

    pub async fn execute(&self) -> Result<Vec<AnnouncementDraft>, Unavailable> {
        self.drafts.find_all().await
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{identity::AccountId, notification::AnnouncementDraftId};

    use super::*;
    use crate::in_memory::InMemoryAnnouncementDraftRepository;

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn draft(id: u128, minute: u64) -> AnnouncementDraft {
        AnnouncementDraft::new(
            AnnouncementDraftId::from_u128(id),
            AccountId::from_u128(5 + id),
            format!("題{id}"),
            "本文",
            None,
            at(minute),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn lists_every_draft_most_recently_updated_first() {
        let (old, revised, same_time) = (
            draft(1, 10),
            draft(2, 5).with_updated_at(at(20)),
            draft(3, 20),
        );
        let drafts = InMemoryAnnouncementDraftRepository::with_drafts([
            old.clone(),
            revised.clone(),
            same_time.clone(),
        ]);
        let listed = ListAnnouncementDrafts::new(drafts.clone())
            .execute()
            .await
            .unwrap();
        assert_eq!(
            listed,
            [same_time, revised, old],
            "最後に直した時刻の新しい順。同じ時刻は後に作ったものが先。作った本部のアカウントで絞らない"
        );
        assert_eq!(drafts.all().len(), 3, "照会では何も変えない");
    }

    #[tokio::test]
    async fn no_drafts_is_an_empty_list() {
        let listed = ListAnnouncementDrafts::new(InMemoryAnnouncementDraftRepository::default())
            .execute()
            .await
            .unwrap();
        assert!(listed.is_empty());
    }
}
