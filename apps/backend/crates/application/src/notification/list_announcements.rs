use super::{AnnouncementRepository, AnnouncementWithReadCount, NotificationRepository};
use crate::Unavailable;

/// 本部が送ったお知らせの一覧のユースケース（admin-web の `listAnnouncements`。本部だけが呼ぶ）。照会だけで、何も保存しない。
/// 送った時刻の新しい順（同じ時刻は後に送ったものが先）で、どの本部のアカウントが送ったかで絞らない。取り下げたものも載せる。
/// 1 件ごとに、受け取った生徒のうち既読にした数を添える（[`AnnouncementWithReadCount`]）。
/// 誰が呼べるか（役割）は境界（interface）が確かめる。行動イベントは発火しない。
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

    pub async fn execute(&self) -> Result<Vec<AnnouncementWithReadCount>, Unavailable> {
        let announcements = self.announcements.find_all().await?;
        AnnouncementWithReadCount::attach(announcements, &self.notifications).await
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        time::{Duration, SystemTime},
    };

    use domain::{
        catalog::Sport,
        identity::AccountId,
        notification::{
            Announcement, AnnouncementBody, AnnouncementId, AnnouncementTitle, Notification,
            NotificationId, PushNotification,
        },
        scheduling::BookingId,
    };

    use super::*;
    use crate::in_memory::{InMemoryAnnouncementRepository, InMemoryNotificationRepository};

    const HEADQUARTERS: AccountId = AccountId::from_u128(5);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn announcement(id: u128, minute: u64) -> Announcement {
        Announcement::new(
            AnnouncementId::from_u128(id),
            AccountId::from_u128(5 + id),
            AnnouncementTitle::new(format!("題{id}")).unwrap(),
            AnnouncementBody::new("本文").unwrap(),
            (id == 2).then_some(Sport::Tennis),
            3,
            at(minute),
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
        .execute()
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
        .execute()
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
        .execute()
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
        .execute()
        .await
        .unwrap();
        assert!(listed.is_empty());
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
        .execute()
        .await;
        assert!(result.is_err());
    }
}
