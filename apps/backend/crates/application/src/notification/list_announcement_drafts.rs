use domain::notification::{AnnouncementDraft, AnnouncementTitleQuery};

use super::AnnouncementDraftRepository;
use crate::Unavailable;

/// 本部からのお知らせの下書きの一覧のユースケース（admin-web の `listAnnouncementDrafts`。本部だけが呼ぶ）。照会だけで、
/// 何も保存しない。最後に直した時刻の新しい順（同じ時刻は後に作ったものが先）で、どの本部のアカウントが作ったかで
/// 絞らない。題の検索語（[`AnnouncementTitleQuery`]）があれば、題に部分一致する下書きだけに絞る（並べ方は変わらない）。送った下書きは消えているので載らない。誰が呼べるか（役割）は境界（interface）が確かめる。行動イベントは
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

    pub async fn execute(
        &self,
        title: Option<&AnnouncementTitleQuery>,
    ) -> Result<Vec<AnnouncementDraft>, Unavailable> {
        let mut drafts = self.drafts.find_all().await?;
        if let Some(title) = title {
            drafts.retain(|draft| title.matches(draft.title()));
        }
        Ok(drafts)
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
            .execute(None)
            .await
            .unwrap();
        assert_eq!(
            listed,
            [same_time, revised, old],
            "最後に直した時刻の新しい順。同じ時刻は後に作ったものが先。作った本部のアカウントで絞らない"
        );
        assert_eq!(drafts.all().len(), 3, "照会では何も変えない");
    }

    fn titled(id: u128, minute: u64, title: &str) -> AnnouncementDraft {
        AnnouncementDraft::new(
            AnnouncementDraftId::from_u128(id),
            AccountId::from_u128(5),
            title,
            "下書きの本文",
            None,
            at(minute),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn a_title_query_keeps_only_drafts_whose_title_contains_it_in_the_same_order() {
        let (maintenance, app, untitled, night) = (
            titled(1, 10, "メンテナンスのお知らせ"),
            titled(2, 20, "App の更新"),
            titled(3, 30, ""),
            titled(4, 40, "夜間メンテナンス（再）"),
        );
        let drafts = InMemoryAnnouncementDraftRepository::with_drafts([
            maintenance.clone(),
            app.clone(),
            untitled,
            night.clone(),
        ]);
        let list = ListAnnouncementDrafts::new(drafts.clone());
        let search = |q: &str| AnnouncementTitleQuery::new(q).unwrap().unwrap();

        assert_eq!(
            list.execute(Some(&search(" メンテナンス "))).await.unwrap(),
            [night, maintenance],
            "題に部分一致するものだけ。最後に直した時刻の新しい順は変わらない"
        );
        assert_eq!(
            list.execute(Some(&search("app"))).await.unwrap(),
            [app],
            "ASCII の英字は大文字・小文字を区別しない"
        );
        assert!(
            list.execute(Some(&search("本文")))
                .await
                .unwrap()
                .is_empty(),
            "本文では探さない。一致しなければ空の一覧"
        );
        assert_eq!(drafts.all().len(), 4, "照会では何も変えない");
    }

    #[tokio::test]
    async fn no_drafts_is_an_empty_list() {
        let listed = ListAnnouncementDrafts::new(InMemoryAnnouncementDraftRepository::default())
            .execute(None)
            .await
            .unwrap();
        assert!(listed.is_empty());
    }
}
