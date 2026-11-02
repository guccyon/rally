use domain::{notification::Announcement, scheduling::BookingPeriod};
use time::Duration;

use super::{AnnouncementRepository, ListAnnouncementsError, ListAnnouncementsQuery};
use crate::scheduling::calendar_day;

/// 本部が送ったお知らせの一覧のユースケース（admin-web の `listAnnouncements`。本部だけが呼ぶ）。照会だけで、何も保存しない。
/// 送った時刻の新しい順（同じ時刻は後に送ったものが先）で、どの本部のアカウントが送ったかで絞らない。取り下げたものも載せる。
/// 誰が呼べるか（役割）は境界（interface）が確かめる。行動イベントは発火しない。
///
/// 条件（[`ListAnnouncementsQuery`]）の日付を両方省けば今までどおりすべて。どちらかがあれば、送った日付（日本時間。
/// [`calendar_day`] で区切る）が `from` から `to` まで（両端を含む）のものだけに絞る。`from` だけなら `from` から、`to`
/// だけなら `to` までの [`BookingPeriod::DEFAULT_DAYS`] 日（キャンセルされた予約の一覧と同じ）。期間の規則（`to` が
/// `from` より前でない・[`BookingPeriod::MAX_DAYS`] 日まで）は本部が見る予約の一覧と同じ domain の [`BookingPeriod`]。
pub struct ListAnnouncements<A> {
    announcements: A,
}

impl<A> ListAnnouncements<A>
where
    A: AnnouncementRepository,
{
    pub fn new(announcements: A) -> Self {
        Self { announcements }
    }

    pub async fn execute(
        &self,
        query: ListAnnouncementsQuery,
    ) -> Result<Vec<Announcement>, ListAnnouncementsError> {
        let span = Duration::days(i64::from(BookingPeriod::DEFAULT_DAYS) - 1);
        let (from, to) = match (query.from, query.to) {
            (None, None) => return Ok(self.announcements.find_all().await?),
            (Some(from), Some(to)) => (from, to),
            (Some(from), None) => (from, from.saturating_add(span)),
            (None, Some(to)) => (to.saturating_sub(span), to),
        };
        let period = BookingPeriod::new(calendar_day::start(from), calendar_day::end(to))?;
        let mut announcements = self
            .announcements
            .find_sent_between(period.starts_at(), period.ends_at())
            .await?;
        announcements.retain(|announcement| announcement.is_sent_in(&period));
        Ok(announcements)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration as StdDuration, SystemTime};

    use domain::{
        catalog::Sport,
        identity::AccountId,
        notification::{AnnouncementBody, AnnouncementId, AnnouncementTitle},
        scheduling::SchedulingError,
    };
    use time::{Date, Month};

    use super::*;
    use crate::{Unavailable, in_memory::InMemoryAnnouncementRepository};

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + StdDuration::from_secs(minute * 60)
    }

    fn nov(day: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, day).unwrap()
    }

    fn announcement_at(id: u128, sent_at: SystemTime) -> Announcement {
        Announcement::new(
            AnnouncementId::from_u128(id),
            AccountId::from_u128(5 + id),
            AnnouncementTitle::new(format!("題{id}")).unwrap(),
            AnnouncementBody::new("本文").unwrap(),
            (id == 2).then_some(Sport::Tennis),
            1,
            sent_at,
        )
        .unwrap()
    }

    fn announcement(id: u128, minute: u64) -> Announcement {
        announcement_at(id, at(minute))
    }

    async fn list(
        repository: &InMemoryAnnouncementRepository,
        from: Option<Date>,
        to: Option<Date>,
    ) -> Result<Vec<Announcement>, ListAnnouncementsError> {
        ListAnnouncements::new(repository.clone())
            .execute(ListAnnouncementsQuery { from, to })
            .await
    }

    #[tokio::test]
    async fn lists_every_announcement_newest_first() {
        let (old, new, same_time) = (
            announcement(1, 10),
            announcement(2, 20),
            announcement(3, 20),
        );
        let repository = InMemoryAnnouncementRepository::with_announcements([
            old.clone(),
            new.clone(),
            same_time.clone(),
        ]);
        let listed = list(&repository, None, None).await.unwrap();
        assert_eq!(
            listed,
            [same_time, new, old],
            "新しい順。同じ時刻は後に残したものが先。送った本部のアカウントで絞らない"
        );
        assert_eq!(repository.all().len(), 3, "照会では何も変えない");
    }

    #[tokio::test]
    async fn no_announcements_is_an_empty_list() {
        let repository = InMemoryAnnouncementRepository::default();
        assert!(list(&repository, None, None).await.unwrap().is_empty());
        assert!(
            list(&repository, Some(nov(1)), Some(nov(30)))
                .await
                .unwrap()
                .is_empty()
        );
    }

    /// 11/1〜11/3 の日本時間の境目の前後に送ったお知らせ（1: 10/31 の最後、2: 11/1 の始まり、3: 11/3 の最後、
    /// 4: 11/4 の始まり）。取り下げたもの（3）も含む。
    fn around_the_period() -> (InMemoryAnnouncementRepository, [Announcement; 4]) {
        let nov1 = calendar_day::start(nov(1));
        let nov4 = calendar_day::start(nov(4));
        let nanos = StdDuration::from_nanos(1);
        let mut withdrawn = announcement_at(3, nov4 - nanos);
        withdrawn
            .withdraw(AccountId::from_u128(9), nov4 + nanos)
            .unwrap();
        let sent = [
            announcement_at(1, nov1 - nanos),
            announcement_at(2, nov1),
            withdrawn,
            announcement_at(4, nov4),
        ];
        (
            InMemoryAnnouncementRepository::with_announcements(sent.clone()),
            sent,
        )
    }

    #[tokio::test]
    async fn the_period_includes_both_dates_in_japan_time() {
        let (repository, [_, first_day, last_day, _]) = around_the_period();
        assert_eq!(
            list(&repository, Some(nov(1)), Some(nov(3))).await.unwrap(),
            [last_day.clone(), first_day.clone()],
            "from の 0 時から to の翌日 0 時の手前まで（日本時間）。新しい順で、取り下げたものも載る"
        );
        assert_eq!(
            list(&repository, Some(nov(1)), Some(nov(1))).await.unwrap(),
            [first_day],
            "1 日だけでもよい"
        );
    }

    #[tokio::test]
    async fn one_missing_date_is_seven_days_from_the_other() {
        let (repository, [before, first_day, last_day, after]) = around_the_period();
        assert_eq!(
            list(&repository, Some(nov(1)), None).await.unwrap(),
            [after.clone(), last_day.clone(), first_day.clone()],
            "from だけなら from から 7 日目（11/7）まで"
        );
        assert_eq!(
            list(&repository, None, Some(nov(3))).await.unwrap(),
            [last_day, first_day, before],
            "to だけなら to の 6 日前（10/28）から"
        );
        assert!(
            list(&repository, Some(nov(8)), None)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn periods_breaking_the_rule_are_invalid() {
        let (repository, _) = around_the_period();
        assert!(matches!(
            list(&repository, Some(nov(3)), Some(nov(2))).await,
            Err(ListAnnouncementsError::Invalid(
                SchedulingError::BookingPeriodEndsBeforeItStarts
            ))
        ));
        assert_eq!(
            list(
                &repository,
                Some(nov(1)),
                Some(Date::from_calendar_date(2026, Month::December, 1).unwrap())
            )
            .await
            .unwrap()
            .len(),
            3,
            "31 日（11/1〜12/1 は 31 日）まではよい"
        );
        assert!(matches!(
            list(
                &repository,
                Some(nov(1)),
                Some(Date::from_calendar_date(2026, Month::December, 2).unwrap())
            )
            .await,
            Err(ListAnnouncementsError::Invalid(
                SchedulingError::BookingPeriodTooLong { max_days: 31 }
            ))
        ));
    }

    /// 読めない保存先
    struct BrokenAnnouncements;

    impl AnnouncementRepository for BrokenAnnouncements {
        fn next_id(&self) -> AnnouncementId {
            AnnouncementId::from_u128(1)
        }

        async fn insert(&self, _announcement: Announcement) -> Result<(), Unavailable> {
            unreachable!()
        }

        async fn find(&self, _id: AnnouncementId) -> Result<Option<Announcement>, Unavailable> {
            unreachable!()
        }

        async fn update(&self, _announcement: Announcement) -> Result<(), Unavailable> {
            unreachable!()
        }

        async fn find_all(&self) -> Result<Vec<Announcement>, Unavailable> {
            Err(Unavailable::new("壊れている"))
        }

        async fn find_sent_between(
            &self,
            _starts_at: SystemTime,
            _ends_at: SystemTime,
        ) -> Result<Vec<Announcement>, Unavailable> {
            Err(Unavailable::new("壊れている"))
        }

        async fn find_by_ids(
            &self,
            _ids: &std::collections::BTreeSet<AnnouncementId>,
        ) -> Result<std::collections::BTreeMap<AnnouncementId, Announcement>, Unavailable> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn a_broken_store_is_unavailable() {
        for (from, to) in [(None, None), (Some(nov(1)), None)] {
            assert!(matches!(
                ListAnnouncements::new(BrokenAnnouncements)
                    .execute(ListAnnouncementsQuery { from, to })
                    .await,
                Err(ListAnnouncementsError::Unavailable(_))
            ));
        }
    }
}
