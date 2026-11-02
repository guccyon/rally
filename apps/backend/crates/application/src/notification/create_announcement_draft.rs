use domain::{catalog::Sport, identity::AccountId, notification::AnnouncementDraft};

use super::{AnnouncementDraftError, AnnouncementDraftRepository};
use crate::clock::Clock;

/// 本部からのお知らせの下書きの作成のユースケース（admin-web の `createAnnouncementDraft`。本部だけが呼ぶ）。誰が呼べるか
/// （本部だけ）は境界（interface）が確かめる。
///
/// domain の [`AnnouncementDraft::new`] で下書きを作り（作った時刻は今）、保存して返す。書きかけなので題・本文は空でも
/// よく、上限を超えれば [`AnnouncementDraftError::Invalid`] で何も残さない。下書きは生徒に届かない（お知らせも
/// プッシュ通知も作らない）。行動イベントも境界間イベントも発火しない（本部の運営の操作）。
pub struct CreateAnnouncementDraft<D, C> {
    drafts: D,
    clock: C,
}

impl<D, C> CreateAnnouncementDraft<D, C>
where
    D: AnnouncementDraftRepository,
    C: Clock,
{
    pub fn new(drafts: D, clock: C) -> Self {
        Self { drafts, clock }
    }

    /// `created_by` は認証した本部のアカウント。`sport` が `None` なら生徒全員に送る下書き。
    pub async fn execute(
        &self,
        created_by: AccountId,
        title: String,
        body: String,
        sport: Option<Sport>,
    ) -> Result<AnnouncementDraft, AnnouncementDraftError> {
        let draft = AnnouncementDraft::new(
            self.drafts.next_id(),
            created_by,
            title,
            body,
            sport,
            self.clock.now(),
        )
        .map_err(AnnouncementDraftError::Invalid)?;
        self.drafts.insert(draft.clone()).await?;
        Ok(draft)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::notification::NotificationError;

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryAnnouncementDraftRepository};

    const HEADQUARTERS: AccountId = AccountId::from_u128(5);

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    fn use_case() -> CreateAnnouncementDraft<InMemoryAnnouncementDraftRepository, FixedClock> {
        CreateAnnouncementDraft::new(
            InMemoryAnnouncementDraftRepository::default(),
            FixedClock(now()),
        )
    }

    #[tokio::test]
    async fn headquarters_saves_a_draft_now() {
        let use_case = use_case();
        let draft = use_case
            .execute(
                HEADQUARTERS,
                "メンテナンスのお知らせ".to_string(),
                String::new(),
                Some(Sport::Swimming),
            )
            .await
            .unwrap();
        assert_eq!(draft.created_by(), HEADQUARTERS);
        assert_eq!(draft.title(), "メンテナンスのお知らせ");
        assert_eq!(draft.body(), "", "書きかけの空の本文も残せる");
        assert_eq!(draft.sport(), Some(Sport::Swimming));
        assert_eq!(draft.created_at(), now());
        assert_eq!(draft.updated_at(), now());
        assert_eq!(use_case.drafts.all(), [draft]);
    }

    #[tokio::test]
    async fn a_too_long_draft_is_rejected_and_records_nothing() {
        let use_case = use_case();
        let result = use_case
            .execute(HEADQUARTERS, "あ".repeat(51), String::new(), None)
            .await;
        assert!(matches!(
            result,
            Err(AnnouncementDraftError::Invalid(
                NotificationError::AnnouncementTitleTooLong { max: 50 }
            ))
        ));
        assert!(use_case.drafts.all().is_empty());
    }
}
