use domain::{
    catalog::Sport,
    notification::{AnnouncementDraft, AnnouncementTitleQuery},
};

use super::AnnouncementDraftRepository;
use crate::Unavailable;

/// 本部からのお知らせの下書きの一覧のユースケース（admin-web の `listAnnouncementDrafts`。本部だけが呼ぶ）。照会だけで、
/// 何も保存しない。最後に直した時刻の新しい順（同じ時刻は後に作ったものが先）で、どの本部のアカウントが作ったかで
/// 絞らない。題の検索語（[`AnnouncementTitleQuery`]）があれば、題に部分一致する下書きだけに絞る（並べ方は変わらない）。スポーツ `sport` があれば、そのスポーツの生徒に向けた
/// 下書きだけに絞る（生徒全員に向けた下書きは入らない。判定は送ったお知らせの一覧と同じ domain の
/// [`AnnouncementDraft::is_in_sport`]）。題とスポーツは組み合わせられ、並べ方は変わらない。送った下書きは消えているので
/// 載らない。誰が呼べるか（役割）は境界（interface）が確かめる。行動イベントは発火しない。
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
        sport: Option<Sport>,
    ) -> Result<Vec<AnnouncementDraft>, Unavailable> {
        let mut drafts = self.drafts.find_all().await?;
        if let Some(title) = title {
            drafts.retain(|draft| title.matches(draft.title()));
        }
        drafts.retain(|draft| draft.is_in_sport(sport));
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
            .execute(None, None)
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
            list.execute(Some(&search(" メンテナンス ")), None)
                .await
                .unwrap(),
            [night, maintenance],
            "題に部分一致するものだけ。最後に直した時刻の新しい順は変わらない"
        );
        assert_eq!(
            list.execute(Some(&search("app")), None).await.unwrap(),
            [app],
            "ASCII の英字は大文字・小文字を区別しない"
        );
        assert!(
            list.execute(Some(&search("本文")), None)
                .await
                .unwrap()
                .is_empty(),
            "本文では探さない。一致しなければ空の一覧"
        );
        assert_eq!(drafts.all().len(), 4, "照会では何も変えない");
    }

    fn for_sport(id: u128, minute: u64, title: &str, sport: Option<Sport>) -> AnnouncementDraft {
        AnnouncementDraft::new(
            AnnouncementDraftId::from_u128(id),
            AccountId::from_u128(5),
            title,
            "下書きの本文",
            sport,
            at(minute),
        )
        .unwrap()
    }

    fn for_sports() -> (InMemoryAnnouncementDraftRepository, [AnnouncementDraft; 4]) {
        let drafts = [
            for_sport(1, 10, "テニスの大会", Some(Sport::Tennis)),
            for_sport(2, 20, "ゴルフの大会", Some(Sport::Golf)),
            for_sport(3, 30, "テニスの大会と更新", None),
            for_sport(4, 40, "テニスの大会の結果", Some(Sport::Tennis)),
        ];
        (
            InMemoryAnnouncementDraftRepository::with_drafts(drafts.clone()),
            drafts,
        )
    }

    #[tokio::test]
    async fn a_sport_keeps_only_drafts_for_that_sport_in_the_same_order() {
        let (repository, [first, golf, everyone, last]) = for_sports();
        let list = ListAnnouncementDrafts::new(repository.clone());
        assert_eq!(
            list.execute(None, Some(Sport::Tennis)).await.unwrap(),
            [last.clone(), first.clone()],
            "そのスポーツの生徒に向けた下書きだけ。生徒全員に向けた下書きは入らない。並べ方は変わらない"
        );
        assert_eq!(
            list.execute(None, Some(Sport::Golf)).await.unwrap(),
            std::slice::from_ref(&golf)
        );
        assert!(
            list.execute(None, Some(Sport::Swimming))
                .await
                .unwrap()
                .is_empty(),
            "そのスポーツの下書きがなければ空の一覧"
        );
        assert_eq!(
            list.execute(None, None).await.unwrap(),
            [last, everyone, golf, first],
            "付けなければ今までどおりすべて"
        );
        assert_eq!(repository.all().len(), 4, "照会では何も変えない");
    }

    #[tokio::test]
    async fn a_sport_combines_with_the_title_query() {
        let (repository, [first, _, _, last]) = for_sports();
        let list = ListAnnouncementDrafts::new(repository);
        let search = |q: &str| AnnouncementTitleQuery::new(q).unwrap().unwrap();
        assert_eq!(
            list.execute(Some(&search("大会")), Some(Sport::Tennis))
                .await
                .unwrap(),
            [last, first],
            "題とスポーツの両方を満たすものだけ。並べ方は変わらない"
        );
        assert_eq!(
            list.execute(Some(&search("更新")), Some(Sport::Tennis))
                .await
                .unwrap(),
            [],
            "題が合っても生徒全員に向けた下書きは入らない"
        );
        assert_eq!(
            list.execute(Some(&search("テニスの大会")), Some(Sport::Golf))
                .await
                .unwrap(),
            [],
            "スポーツが違えば題が合っても入らない"
        );
    }

    #[tokio::test]
    async fn no_drafts_is_an_empty_list() {
        let listed = ListAnnouncementDrafts::new(InMemoryAnnouncementDraftRepository::default())
            .execute(None, None)
            .await
            .unwrap();
        assert!(listed.is_empty());
    }
}
