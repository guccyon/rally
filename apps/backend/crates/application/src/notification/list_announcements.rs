use domain::scheduling::BookingPeriod;
use time::Duration;

use super::{
    AnnouncementRepository, AnnouncementWithReadCount, ListAnnouncementsError,
    ListAnnouncementsQuery, NotificationRepository,
};
use crate::scheduling::calendar_day;

/// 本部が送ったお知らせの一覧のユースケース（admin-web の `listAnnouncements`。本部だけが呼ぶ）。照会だけで、何も保存しない。
/// 送った時刻の新しい順（同じ時刻は後に送ったものが先）で、どの本部のアカウントが送ったかで絞らない。取り下げたものも載せる。
/// 1 件ごとに、受け取った生徒のうち既読にした数を添える（[`AnnouncementWithReadCount`]）。
/// 誰が呼べるか（役割）は境界（interface）が確かめる。行動イベントは発火しない。
///
/// 条件（[`ListAnnouncementsQuery`]）の日付を両方省けば今までどおりすべて。どちらかがあれば、送った日付（日本時間。
/// [`calendar_day`] で区切る）が `from` から `to` まで（両端を含む）のものだけに絞る（既読にした数は絞ったものにだけ添える）。
/// `from` だけなら `from` から、`to` だけなら `to` までの [`BookingPeriod::DEFAULT_DAYS`] 日（キャンセルされた予約の一覧と
/// 同じ）。期間の規則（`to` が `from` より前でない・[`BookingPeriod::MAX_DAYS`] 日まで）は本部が見る予約の一覧と同じ
/// domain の [`BookingPeriod`]。
pub struct ListAnnouncements<A, N> {
    announcements: A,
    notifications: N,
}

impl<A, N> ListAnnouncements<A, N>
where
    A: AnnouncementRepository,
    N: NotificationRepository,
{
    pub fn new(announcements: A, notifications: N) -> Self {
        Self {
            announcements,
            notifications,
        }
    }

    pub async fn execute(
        &self,
        query: ListAnnouncementsQuery,
    ) -> Result<Vec<AnnouncementWithReadCount>, ListAnnouncementsError> {
        let span = Duration::days(i64::from(BookingPeriod::DEFAULT_DAYS) - 1);
        let period = match (query.from, query.to) {
            (None, None) => None,
            (Some(from), Some(to)) => Some((from, to)),
            (Some(from), None) => Some((from, from.saturating_add(span))),
            (None, Some(to)) => Some((to.saturating_sub(span), to)),
        };
        let announcements = match period {
            None => self.announcements.find_all().await?,
            Some((from, to)) => {
                let period = BookingPeriod::new(calendar_day::start(from), calendar_day::end(to))?;
                let mut announcements = self
                    .announcements
                    .find_sent_between(period.starts_at(), period.ends_at())
                    .await?;
                announcements.retain(|announcement| announcement.is_sent_in(&period));
                announcements
            }
        };
        Ok(AnnouncementWithReadCount::attach(announcements, &self.notifications).await?)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        time::{Duration as StdDuration, SystemTime},
    };

    use domain::{
        catalog::Sport,
        identity::AccountId,
        notification::{
            Announcement, AnnouncementBody, AnnouncementId, AnnouncementTitle, Notification,
            NotificationId, PushNotification,
        },
        scheduling::{BookingId, SchedulingError},
    };
    use time::{Date, Month};

    use super::*;
    use crate::{
        Unavailable,
        in_memory::{InMemoryAnnouncementRepository, InMemoryNotificationRepository},
    };

    const HEADQUARTERS: AccountId = AccountId::from_u128(5);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + StdDuration::from_secs(minute * 60)
    }

    fn nov(day: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, day).unwrap()
    }

    fn announcement(id: u128, minute: u64) -> Announcement {
        announcement_at(id, at(minute))
    }

    fn announcement_at(id: u128, sent_at: SystemTime) -> Announcement {
        Announcement::new(
            AnnouncementId::from_u128(id),
            AccountId::from_u128(5 + id),
            AnnouncementTitle::new(format!("題{id}")).unwrap(),
            AnnouncementBody::new("本文").unwrap(),
            (id == 2).then_some(Sport::Tennis),
            3,
            sent_at,
        )
        .unwrap()
    }

    /// 生徒 `student` が受け取った `announcement` のお知らせ。`read` の時刻に読んだ（`None` なら読んでいない）。
    fn received(
        id: u128,
        announcement: &Announcement,
        student: u128,
        read: Option<u64>,
    ) -> Notification {
        let mut notification = Notification::new(
            NotificationId::from_u128(id),
            announcement.push_to(AccountId::from_u128(student)),
            announcement.sent_at(),
        );
        if let Some(minute) = read {
            notification.mark_read(at(minute));
        }
        notification
    }

    fn listed(announcement: &Announcement, read_count: u32) -> AnnouncementWithReadCount {
        AnnouncementWithReadCount {
            announcement: announcement.clone(),
            read_count,
        }
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
        let listed_now = ListAnnouncements::new(
            repository.clone(),
            InMemoryNotificationRepository::default(),
        )
        .execute(ListAnnouncementsQuery::default())
        .await
        .unwrap();
        assert_eq!(
            listed_now,
            [listed(&same_time, 0), listed(&new, 0), listed(&old, 0)],
            "新しい順。同じ時刻は後に残したものが先。送った本部のアカウントで絞らない"
        );
        assert_eq!(repository.all().len(), 3, "照会では何も変えない");
    }

    #[tokio::test]
    async fn counts_the_students_who_read_each_announcement() {
        let (first, second) = (announcement(1, 10), announcement(2, 20));
        let notifications = InMemoryNotificationRepository::with_notifications([
            received(1, &first, 11, Some(30)),
            received(2, &first, 12, Some(40)),
            received(3, &first, 13, None),
            received(4, &second, 11, Some(30)),
            received(5, &second, 12, None),
            received(6, &second, 13, None),
            // 本部からのお知らせでないものは数えない
            Notification::restore(
                NotificationId::from_u128(7),
                PushNotification::BookingConfirmed {
                    student_id: AccountId::from_u128(11),
                    coach_id: AccountId::from_u128(9),
                    booking_id: BookingId::from_u128(1),
                },
                at(10),
                Some(at(30)),
            )
            .unwrap(),
        ]);
        let listed_now = ListAnnouncements::new(
            InMemoryAnnouncementRepository::with_announcements([first.clone(), second.clone()]),
            notifications.clone(),
        )
        .execute(ListAnnouncementsQuery::default())
        .await
        .unwrap();
        assert_eq!(
            listed_now,
            [listed(&second, 1), listed(&first, 2)],
            "受け取った生徒のうち既読にした数"
        );
        assert_eq!(
            notifications.all().iter().filter(|n| n.is_read()).count(),
            4,
            "照会では既読を変えない"
        );
    }

    #[tokio::test]
    async fn a_withdrawn_announcement_counts_only_the_reads_until_the_withdrawal() {
        let mut withdrawn = announcement(1, 10);
        withdrawn.withdraw(HEADQUARTERS, at(20)).unwrap();
        let notifications = InMemoryNotificationRepository::with_notifications([
            received(1, &withdrawn, 11, Some(15)),
            // 取り下げた時刻ちょうどに読んだものは数える
            received(2, &withdrawn, 12, Some(20)),
            // 取り下げたあとに `markMyNotificationsRead` で既読になったものは数えない（生徒の一覧に出ていない）
            received(3, &withdrawn, 13, Some(30)),
            received(4, &withdrawn, 14, None),
        ]);
        let listed_now = ListAnnouncements::new(
            InMemoryAnnouncementRepository::with_announcements([withdrawn.clone()]),
            notifications.clone(),
        )
        .execute(ListAnnouncementsQuery::default())
        .await
        .unwrap();
        assert_eq!(
            listed_now,
            [listed(&withdrawn, 2)],
            "取り下げた時刻までに既読になったものだけ"
        );
        assert_eq!(
            notifications.all().iter().filter(|n| n.is_read()).count(),
            3,
            "既読そのものは変えない"
        );
    }

    #[tokio::test]
    async fn no_announcements_is_an_empty_list() {
        let listed = ListAnnouncements::new(
            InMemoryAnnouncementRepository::default(),
            InMemoryNotificationRepository::default(),
        )
        .execute(ListAnnouncementsQuery::default())
        .await
        .unwrap();
        assert!(listed.is_empty());
    }

    /// 既読の数を数える保存先は空のまま、条件 `from`・`to` で一覧にしたお知らせ（既読にした数は除く）。
    async fn list(
        repository: &InMemoryAnnouncementRepository,
        from: Option<Date>,
        to: Option<Date>,
    ) -> Result<Vec<Announcement>, ListAnnouncementsError> {
        ListAnnouncements::new(
            repository.clone(),
            InMemoryNotificationRepository::default(),
        )
        .execute(ListAnnouncementsQuery { from, to })
        .await
        .map(|listed| listed.into_iter().map(|l| l.announcement).collect())
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

    #[tokio::test]
    async fn read_counts_are_attached_to_the_announcements_in_the_period() {
        let (repository, [before, first_day, ..]) = around_the_period();
        let notifications = InMemoryNotificationRepository::with_notifications([
            received(1, &before, 11, Some(30_000_000)),
            received(2, &first_day, 11, Some(30_000_000)),
            received(3, &first_day, 12, None),
        ]);
        let listed_now = ListAnnouncements::new(repository, notifications)
            .execute(ListAnnouncementsQuery {
                from: Some(nov(1)),
                to: Some(nov(1)),
            })
            .await
            .unwrap();
        assert_eq!(
            listed_now,
            [listed(&first_day, 1)],
            "期間で絞ったものにだけ既読にした数を添える"
        );
    }

    /// 既読の数を読めない保存先
    struct BrokenNotifications;

    impl NotificationRepository for BrokenNotifications {
        fn next_id(&self) -> NotificationId {
            unreachable!()
        }

        async fn insert(&self, _notification: Notification) -> Result<(), Unavailable> {
            unreachable!()
        }

        async fn find_by_recipient(
            &self,
            _recipient: AccountId,
        ) -> Result<Vec<Notification>, Unavailable> {
            unreachable!()
        }

        async fn find_announcements_by_recipient(
            &self,
            _recipient: AccountId,
            _limit: usize,
        ) -> Result<Vec<Notification>, Unavailable> {
            unreachable!()
        }

        async fn mark_all_read(
            &self,
            _recipient: AccountId,
            _at: SystemTime,
        ) -> Result<(), Unavailable> {
            unreachable!()
        }

        async fn count_read_by_announcement(
            &self,
            _read_until: &BTreeMap<AnnouncementId, Option<SystemTime>>,
        ) -> Result<BTreeMap<AnnouncementId, u32>, Unavailable> {
            Err(Unavailable::new("壊れている"))
        }
    }

    #[tokio::test]
    async fn reads_that_cannot_be_counted_are_unavailable() {
        let result = ListAnnouncements::new(
            InMemoryAnnouncementRepository::with_announcements([announcement(1, 10)]),
            BrokenNotifications,
        )
        .execute(ListAnnouncementsQuery::default())
        .await;
        assert!(result.is_err());
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
                ListAnnouncements::new(
                    BrokenAnnouncements,
                    InMemoryNotificationRepository::default()
                )
                .execute(ListAnnouncementsQuery { from, to })
                .await,
                Err(ListAnnouncementsError::Unavailable(_))
            ));
        }
    }
}
