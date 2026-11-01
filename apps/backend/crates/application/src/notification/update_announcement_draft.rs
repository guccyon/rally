use domain::{
    catalog::Sport,
    notification::{AnnouncementDraft, AnnouncementDraftId},
};

use super::{AnnouncementDraftError, AnnouncementDraftRepository};
use crate::clock::Clock;

/// 本部からのお知らせの下書きの書き直しのユースケース（admin-web の `updateAnnouncementDraft`。本部だけが呼ぶ）。誰が
/// 呼べるか（本部だけ）は境界（interface）が確かめる。どの本部のアカウントが作った下書きでも直せる。
///
/// 下書きを読み、domain の [`AnnouncementDraft::revise`] で題・本文・対象のスポーツをまとめて書き換えて（最後に直した
/// 時刻は今）保存し、直したあとの下書きを返す。ない下書きは [`AnnouncementDraftError::DraftNotFound`]、上限を超えれば
/// [`AnnouncementDraftError::Invalid`] で、どちらも何も変えない。行動イベントも境界間イベントも発火しない。
pub struct UpdateAnnouncementDraft<D, C> {
    drafts: D,
    clock: C,
}

impl<D, C> UpdateAnnouncementDraft<D, C>
where
    D: AnnouncementDraftRepository,
    C: Clock,
{
    pub fn new(drafts: D, clock: C) -> Self {
        Self { drafts, clock }
    }

    pub async fn execute(
        &self,
        draft_id: AnnouncementDraftId,
        title: String,
        body: String,
        sport: Option<Sport>,
    ) -> Result<AnnouncementDraft, AnnouncementDraftError> {
        let mut draft = self
            .drafts
            .find(draft_id)
            .await?
            .ok_or(AnnouncementDraftError::DraftNotFound)?;
        draft
            .revise(title, body, sport, self.clock.now())
            .map_err(AnnouncementDraftError::Invalid)?;
        if !self.drafts.update(draft.clone()).await? {
            // 読んでから書くまでに消された
            return Err(AnnouncementDraftError::DraftNotFound);
        }
        Ok(draft)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{identity::AccountId, notification::NotificationError};

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryAnnouncementDraftRepository};

    const DRAFT: AnnouncementDraftId = AnnouncementDraftId::from_u128(7);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn written() -> AnnouncementDraft {
        AnnouncementDraft::new(DRAFT, AccountId::from_u128(5), "題", "", None, at(10)).unwrap()
    }

    fn use_case(
        drafts: InMemoryAnnouncementDraftRepository,
    ) -> UpdateAnnouncementDraft<InMemoryAnnouncementDraftRepository, FixedClock> {
        UpdateAnnouncementDraft::new(drafts, FixedClock(at(20)))
    }

    #[tokio::test]
    async fn headquarters_revises_a_draft_now() {
        let drafts = InMemoryAnnouncementDraftRepository::with_drafts([written()]);
        let revised = use_case(drafts.clone())
            .execute(
                DRAFT,
                "新しい題".to_string(),
                "本文".to_string(),
                Some(Sport::Tennis),
            )
            .await
            .unwrap();
        assert_eq!(revised.title(), "新しい題");
        assert_eq!(revised.body(), "本文");
        assert_eq!(revised.sport(), Some(Sport::Tennis));
        assert_eq!(revised.updated_at(), at(20));
        assert_eq!(revised.created_at(), at(10), "作った時刻は変えない");
        assert_eq!(drafts.all(), [revised], "直したものを残す");
    }

    #[tokio::test]
    async fn a_missing_draft_is_not_found() {
        let drafts = InMemoryAnnouncementDraftRepository::with_drafts([written()]);
        let result = use_case(drafts.clone())
            .execute(
                AnnouncementDraftId::from_u128(8),
                "題".to_string(),
                String::new(),
                None,
            )
            .await;
        assert!(matches!(result, Err(AnnouncementDraftError::DraftNotFound)));
        assert_eq!(drafts.all(), [written()], "何も変えない");
    }

    #[tokio::test]
    async fn a_too_long_revision_changes_nothing() {
        let drafts = InMemoryAnnouncementDraftRepository::with_drafts([written()]);
        let result = use_case(drafts.clone())
            .execute(DRAFT, "題".to_string(), "あ".repeat(2001), None)
            .await;
        assert!(matches!(
            result,
            Err(AnnouncementDraftError::Invalid(
                NotificationError::AnnouncementBodyTooLong { max: 2000 }
            ))
        ));
        assert_eq!(drafts.all(), [written()]);
    }
}
