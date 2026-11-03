use domain::notification::AnnouncementDraft;

use super::{
    AnnouncementDraftRepository, ListAnnouncementDraftsError, ListAnnouncementDraftsQuery,
};
use crate::scheduling::calendar_day;

/// 本部からのお知らせの下書きの一覧のユースケース（admin-web の `listAnnouncementDrafts`。本部だけが呼ぶ）。照会だけで、
/// 何も保存しない。最後に直した時刻の新しい順（同じ時刻は後に作ったものが先）で、どの本部のアカウントが作ったかで
/// 絞らない。送った下書きは消えているので載らない。誰が呼べるか（役割）は境界（interface）が確かめる。行動イベントは
/// 発火しない。
///
/// 条件（[`ListAnnouncementDraftsQuery`]）をすべて省けば今までどおりすべて。日付 `from`・`to` のどちらかがあれば、最後に
/// 直した日付（日本時間。[`calendar_day`] で区切る）が `from` から `to` まで（両端を含む）の下書きだけに絞る。`from` だけ
/// なら `from` から、`to` だけなら `to` までの 7 日で、期間の規則（`to` が `from` より前でない・31 日まで）も含め送った
/// お知らせの一覧と同じ [`calendar_day::period_ending`]（判定は domain の [`AnnouncementDraft::is_updated_in`]）。
/// 題の検索語（[`ListAnnouncementDraftsQuery::title`]）があれば、題に部分一致する下書きだけに絞る。スポーツ
/// （[`ListAnnouncementDraftsQuery::sport`]）があれば、そのスポーツの生徒に向けた下書きだけに絞る（生徒全員に向けた
/// 下書きは入らない。判定は送ったお知らせの一覧と同じ domain の [`AnnouncementDraft::is_in_sport`]）。期間・題・スポーツは
/// 組み合わせられ、並べ方は変わらない。
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
        query: ListAnnouncementDraftsQuery,
    ) -> Result<Vec<AnnouncementDraft>, ListAnnouncementDraftsError> {
        let period = match (query.from, query.to) {
            (None, None) => None,
            (from, to) => {
                let (_, _, period) = calendar_day::period_ending(from, to, || {
                    unreachable!("from か to のどちらかがあるので今日は読まない")
                })?;
                Some(period)
            }
        };
        let mut drafts = self.drafts.find_all().await?;
        if let Some(period) = &period {
            drafts.retain(|draft| draft.is_updated_in(period));
        }
        if let Some(title) = &query.title {
            drafts.retain(|draft| title.matches(draft.title()));
        }
        drafts.retain(|draft| draft.is_in_sport(query.sport));
        Ok(drafts)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        identity::AccountId,
        notification::{AnnouncementDraftId, AnnouncementTitleQuery},
        scheduling::SchedulingError,
    };
    use time::{Date, Month};

    use super::*;
    use crate::in_memory::InMemoryAnnouncementDraftRepository;

    /// 題の検索語とスポーツだけの条件（期間は省く）。
    fn query(
        title: Option<&AnnouncementTitleQuery>,
        sport: Option<Sport>,
    ) -> ListAnnouncementDraftsQuery {
        ListAnnouncementDraftsQuery {
            title: title.cloned(),
            sport,
            ..Default::default()
        }
    }

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
            .execute(ListAnnouncementDraftsQuery::default())
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
            list.execute(query(Some(&search(" メンテナンス ")), None))
                .await
                .unwrap(),
            [night, maintenance],
            "題に部分一致するものだけ。最後に直した時刻の新しい順は変わらない"
        );
        assert_eq!(
            list.execute(query(Some(&search("app")), None))
                .await
                .unwrap(),
            [app],
            "ASCII の英字は大文字・小文字を区別しない"
        );
        assert!(
            list.execute(query(Some(&search("本文")), None))
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
            list.execute(query(None, Some(Sport::Tennis)))
                .await
                .unwrap(),
            [last.clone(), first.clone()],
            "そのスポーツの生徒に向けた下書きだけ。生徒全員に向けた下書きは入らない。並べ方は変わらない"
        );
        assert_eq!(
            list.execute(query(None, Some(Sport::Golf))).await.unwrap(),
            std::slice::from_ref(&golf)
        );
        assert!(
            list.execute(query(None, Some(Sport::Swimming)))
                .await
                .unwrap()
                .is_empty(),
            "そのスポーツの下書きがなければ空の一覧"
        );
        assert_eq!(
            list.execute(ListAnnouncementDraftsQuery::default())
                .await
                .unwrap(),
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
            list.execute(query(Some(&search("大会")), Some(Sport::Tennis)))
                .await
                .unwrap(),
            [last, first],
            "題とスポーツの両方を満たすものだけ。並べ方は変わらない"
        );
        assert_eq!(
            list.execute(query(Some(&search("更新")), Some(Sport::Tennis)))
                .await
                .unwrap(),
            [],
            "題が合っても生徒全員に向けた下書きは入らない"
        );
        assert_eq!(
            list.execute(query(Some(&search("テニスの大会")), Some(Sport::Golf)))
                .await
                .unwrap(),
            [],
            "スポーツが違えば題が合っても入らない"
        );
    }

    fn nov(day: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, day).unwrap()
    }

    /// 条件 `from`・`to` だけで一覧にした下書き。
    async fn list_between(
        repository: &InMemoryAnnouncementDraftRepository,
        from: Option<Date>,
        to: Option<Date>,
    ) -> Result<Vec<AnnouncementDraft>, ListAnnouncementDraftsError> {
        ListAnnouncementDrafts::new(repository.clone())
            .execute(ListAnnouncementDraftsQuery {
                from,
                to,
                ..Default::default()
            })
            .await
    }

    /// 11/1〜11/3 の日本時間の境目の前後に最後に直した下書き（1: 10/31 の最後、2: 11/1 の始まり、3: 11/3 の最後、
    /// 4: 11/4 の始まり）。どれも作ったのは 1970 年で、直した時刻だけが違う。
    fn around_the_period() -> (InMemoryAnnouncementDraftRepository, [AnnouncementDraft; 4]) {
        let nov1 = calendar_day::start(nov(1));
        let nov4 = calendar_day::start(nov(4));
        let nanos = Duration::from_nanos(1);
        let drafts = [
            for_sport(1, 1, "テニスの大会", Some(Sport::Tennis)).with_updated_at(nov1 - nanos),
            for_sport(2, 2, "テニスの大会", Some(Sport::Tennis)).with_updated_at(nov1),
            for_sport(3, 3, "ゴルフの大会", Some(Sport::Golf)).with_updated_at(nov4 - nanos),
            for_sport(4, 4, "テニスの大会", Some(Sport::Tennis)).with_updated_at(nov4),
        ];
        (
            InMemoryAnnouncementDraftRepository::with_drafts(drafts.clone()),
            drafts,
        )
    }

    #[tokio::test]
    async fn the_period_includes_both_dates_of_the_last_update_in_japan_time() {
        let (repository, [_, first_day, last_day, _]) = around_the_period();
        assert_eq!(
            list_between(&repository, Some(nov(1)), Some(nov(3)))
                .await
                .unwrap(),
            [last_day, first_day.clone()],
            "from の 0 時から to の翌日 0 時の手前まで（日本時間）。最後に直した時刻で決まり、新しい順"
        );
        assert_eq!(
            list_between(&repository, Some(nov(1)), Some(nov(1)))
                .await
                .unwrap(),
            [first_day],
            "1 日だけでもよい"
        );
        assert!(
            list_between(&repository, Some(nov(8)), None)
                .await
                .unwrap()
                .is_empty(),
            "期間に直した下書きがなければ空の一覧"
        );
    }

    #[tokio::test]
    async fn one_missing_date_is_seven_days_from_the_other() {
        let (repository, [before, first_day, last_day, after]) = around_the_period();
        assert_eq!(
            list_between(&repository, Some(nov(1)), None).await.unwrap(),
            [after, last_day.clone(), first_day.clone()],
            "from だけなら from から 7 日目（11/7）まで"
        );
        assert_eq!(
            list_between(&repository, None, Some(nov(3))).await.unwrap(),
            [last_day, first_day, before],
            "to だけなら to の 6 日前（10/28）から"
        );
    }

    #[tokio::test]
    async fn periods_breaking_the_rule_are_invalid() {
        let (repository, _) = around_the_period();
        assert!(matches!(
            list_between(&repository, Some(nov(3)), Some(nov(2))).await,
            Err(ListAnnouncementDraftsError::Invalid(
                SchedulingError::BookingPeriodEndsBeforeItStarts
            ))
        ));
        let dec = |day: u8| Date::from_calendar_date(2026, Month::December, day).unwrap();
        assert_eq!(
            list_between(&repository, Some(nov(1)), Some(dec(1)))
                .await
                .unwrap()
                .len(),
            3,
            "31 日（11/1〜12/1）まではよい"
        );
        assert!(matches!(
            list_between(&repository, Some(nov(1)), Some(dec(2))).await,
            Err(ListAnnouncementDraftsError::Invalid(
                SchedulingError::BookingPeriodTooLong { max_days: 31 }
            ))
        ));
        assert_eq!(repository.all().len(), 4, "照会では何も変えない");
    }

    #[tokio::test]
    async fn a_period_combines_with_the_title_query_and_the_sport() {
        let (repository, [_, first_day, _, _]) = around_the_period();
        let list = ListAnnouncementDrafts::new(repository);
        assert_eq!(
            list.execute(ListAnnouncementDraftsQuery {
                from: Some(nov(1)),
                to: Some(nov(3)),
                title: AnnouncementTitleQuery::new("大会").unwrap(),
                sport: Some(Sport::Tennis),
            })
            .await
            .unwrap(),
            [first_day],
            "期間・題・スポーツのすべてを満たすものだけ"
        );
        assert!(
            list.execute(ListAnnouncementDraftsQuery {
                from: Some(nov(1)),
                to: Some(nov(3)),
                title: AnnouncementTitleQuery::new("結果").unwrap(),
                ..Default::default()
            })
            .await
            .unwrap()
            .is_empty(),
            "期間に入っても題が合わなければ入らない"
        );
    }

    #[tokio::test]
    async fn no_drafts_is_an_empty_list() {
        let listed = ListAnnouncementDrafts::new(InMemoryAnnouncementDraftRepository::default())
            .execute(ListAnnouncementDraftsQuery::default())
            .await
            .unwrap();
        assert!(listed.is_empty());
    }
}
