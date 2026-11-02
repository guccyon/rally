use domain::notification::{AnnouncementDraft, AnnouncementDraftId};

use super::{AnnouncementDraftError, AnnouncementDraftRepository};

/// 本部からのお知らせの下書きの削除のユースケース（admin-web の `deleteAnnouncementDraft`。本部だけが呼ぶ）。誰が呼べるか
/// （本部だけ）は境界（interface）が確かめる。どの本部のアカウントが作った下書きでも消せる。
///
/// 下書きを消し、残った下書きの一覧（[`super::ListAnnouncementDrafts`] と同じ並び）を返す。ない下書きは
/// [`AnnouncementDraftError::DraftNotFound`]。送った記録には触れない。行動イベントも境界間イベントも発火しない。
pub struct DeleteAnnouncementDraft<D> {
    drafts: D,
}

impl<D> DeleteAnnouncementDraft<D>
where
    D: AnnouncementDraftRepository,
{
    pub fn new(drafts: D) -> Self {
        Self { drafts }
    }

    pub async fn execute(
        &self,
        draft_id: AnnouncementDraftId,
    ) -> Result<Vec<AnnouncementDraft>, AnnouncementDraftError> {
        if !self.drafts.delete(draft_id).await? {
            return Err(AnnouncementDraftError::DraftNotFound);
        }
        Ok(self.drafts.find_all().await?)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::identity::AccountId;

    use super::*;
    use crate::in_memory::InMemoryAnnouncementDraftRepository;

    fn draft(id: u128, minute: u64) -> AnnouncementDraft {
        AnnouncementDraft::new(
            AnnouncementDraftId::from_u128(id),
            AccountId::from_u128(5),
            format!("題{id}"),
            "",
            None,
            SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn deletes_the_draft_and_returns_the_rest() {
        let drafts = InMemoryAnnouncementDraftRepository::with_drafts([
            draft(1, 10),
            draft(2, 20),
            draft(3, 30),
        ]);
        let rest = DeleteAnnouncementDraft::new(drafts.clone())
            .execute(AnnouncementDraftId::from_u128(2))
            .await
            .unwrap();
        assert_eq!(rest, [draft(3, 30), draft(1, 10)], "残りを新しい順に");
        assert_eq!(drafts.all(), [draft(1, 10), draft(3, 30)]);
    }

    #[tokio::test]
    async fn a_missing_draft_is_not_found() {
        let drafts = InMemoryAnnouncementDraftRepository::with_drafts([draft(1, 10)]);
        let use_case = DeleteAnnouncementDraft::new(drafts.clone());
        use_case
            .execute(AnnouncementDraftId::from_u128(1))
            .await
            .unwrap();
        let result = use_case.execute(AnnouncementDraftId::from_u128(1)).await;
        assert!(
            matches!(result, Err(AnnouncementDraftError::DraftNotFound)),
            "二度目は消すものがない"
        );
        assert!(drafts.all().is_empty());
    }
}
