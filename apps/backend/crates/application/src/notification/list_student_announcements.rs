use std::collections::BTreeSet;

use domain::{identity::AccountId, notification::ReceivedAnnouncement};

use super::{
    AnnouncementRecipientLookup, AnnouncementRepository, ListStudentAnnouncementsError,
    NotificationRepository,
};
use crate::Unavailable;

/// 本部が見る生徒 1 人の受け取ったお知らせの一覧のユースケース（admin-web の `listStudentAnnouncements`。本部だけが呼ぶ）。
/// 生徒から「お知らせが届いていない」と問い合わせを受けたときに確かめるのに使う。照会だけで、何も保存しない（既読にもしない）。
///
/// 生徒が受け取った本部からのお知らせを、お知らせを作った時刻（送った時刻）の新しい順（同じ時刻は後に残したものが先）に
/// 最大 [`Self::LIMIT`] 件、送った記録を添えて返す（[`ReceivedAnnouncement`]）。取り下げたお知らせも含む（生徒の一覧からは
/// 外れているが、届いたことと既読は残っている）。生徒かは profile に問い合わせる（[`AnnouncementRecipientLookup`]）。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。行動イベントは発火しない。
pub struct ListStudentAnnouncements<N, A, L> {
    notifications: N,
    announcements: A,
    students: L,
}

impl<N, A, L> ListStudentAnnouncements<N, A, L>
where
    N: NotificationRepository,
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
{
    /// 返す件数の上限（admin-web の契約の `StudentAnnouncementList.announcements` の `maxItems`）。
    pub const LIMIT: usize = 50;

    pub fn new(notifications: N, announcements: A, students: L) -> Self {
        Self {
            notifications,
            announcements,
            students,
        }
    }

    pub async fn execute(
        &self,
        student_id: AccountId,
    ) -> Result<Vec<ReceivedAnnouncement>, ListStudentAnnouncementsError> {
        if !self.students.is_student(student_id).await? {
            return Err(ListStudentAnnouncementsError::StudentNotFound);
        }
        let notifications = self
            .notifications
            .find_announcements_by_recipient(student_id, Self::LIMIT)
            .await?;
        let ids: BTreeSet<_> = notifications
            .iter()
            .filter_map(|notification| notification.push().announcement_id())
            .collect();
        let found = if ids.is_empty() {
            Default::default()
        } else {
            self.announcements.find_by_ids(&ids).await?
        };
        Ok(notifications
            .into_iter()
            .map(|notification| {
                let announcement = notification
                    .push()
                    .announcement_id()
                    .and_then(|id| found.get(&id).cloned())
                    .ok_or_else(|| {
                        Unavailable::new(format!(
                            "お知らせ {:?} の本部からのお知らせがない",
                            notification.id()
                        ))
                    })?;
                ReceivedAnnouncement::new(notification, announcement).map_err(Unavailable::new)
            })
            .collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        notification::{
            Announcement, AnnouncementBody, AnnouncementId, AnnouncementTitle, Notification,
            NotificationId, PushNotification,
        },
        scheduling::BookingId,
    };

    use super::*;
    use crate::in_memory::{InMemoryAnnouncementRepository, InMemoryNotificationRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER: AccountId = AccountId::from_u128(2);
    const HEADQUARTERS: AccountId = AccountId::from_u128(5);
    const COACH: AccountId = AccountId::from_u128(9);

    /// 生徒は 1 と 2 だけ。
    struct FakeStudents;

    impl AnnouncementRecipientLookup for FakeStudents {
        async fn find_students(
            &self,
            _: Option<domain::catalog::Sport>,
        ) -> Result<Vec<AccountId>, Unavailable> {
            unreachable!("一覧では受け取る生徒すべては問い合わせない")
        }

        async fn is_student(&self, account_id: AccountId) -> Result<bool, Unavailable> {
            Ok([STUDENT, OTHER].contains(&account_id))
        }
    }

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn announcement(id: u128, minute: u64) -> Announcement {
        Announcement::new(
            AnnouncementId::from_u128(id),
            HEADQUARTERS,
            AnnouncementTitle::new(format!("題{id}")).unwrap(),
            AnnouncementBody::new("本文").unwrap(),
            None,
            2,
            at(minute),
        )
        .unwrap()
    }

    fn received(id: u128, sent: &Announcement, student_id: AccountId) -> Notification {
        Notification::new(
            NotificationId::from_u128(id),
            sent.push_to(student_id),
            sent.sent_at(),
        )
    }

    fn summary(listed: &[ReceivedAnnouncement]) -> Vec<(u128, bool, bool)> {
        listed
            .iter()
            .map(|r| (r.announcement_id().as_u128(), r.is_read(), r.is_withdrawn()))
            .collect()
    }

    #[tokio::test]
    async fn lists_the_announcements_the_student_received_newest_first_with_read_and_withdrawn() {
        let (old, new, same_time) = (
            announcement(1, 10),
            announcement(2, 20),
            announcement(3, 20),
        );
        let mut withdrawn = announcement(4, 30);
        withdrawn.withdraw(HEADQUARTERS, at(40)).unwrap();
        let mut read_old = received(11, &old, STUDENT);
        read_old.mark_read(at(15));
        let notifications = InMemoryNotificationRepository::with_notifications([
            read_old,
            received(12, &new, STUDENT),
            received(13, &same_time, STUDENT),
            received(14, &withdrawn, STUDENT),
            received(15, &new, OTHER),
            Notification::new(
                NotificationId::from_u128(16),
                PushNotification::BookingConfirmed {
                    student_id: STUDENT,
                    coach_id: COACH,
                    booking_id: BookingId::from_u128(1),
                },
                at(50),
            ),
        ]);
        let announcements =
            InMemoryAnnouncementRepository::with_announcements([old, new, same_time, withdrawn]);
        let listed =
            ListStudentAnnouncements::new(notifications.clone(), announcements, FakeStudents)
                .execute(STUDENT)
                .await
                .unwrap();
        assert_eq!(
            summary(&listed),
            [
                (4, false, true),
                (3, false, false),
                (2, false, false),
                (1, true, false)
            ],
            "新しい順（同じ時刻は後に残したものが先）。取り下げたものも載る。ほかの生徒の分と本部からのお知らせでないものは載せない"
        );
        assert_eq!(listed[1].title().as_str(), "題3");
        assert_eq!(listed[1].sent_at(), at(20));
        assert_eq!(
            notifications.all().iter().filter(|n| n.is_read()).count(),
            1,
            "照会では既読にしない"
        );
    }

    #[tokio::test]
    async fn lists_at_most_the_newest_fifty() {
        let sent: Vec<Announcement> = (1..=51).map(|id| announcement(id, id as u64)).collect();
        let notifications = InMemoryNotificationRepository::with_notifications(
            sent.iter()
                .map(|a| received(100 + a.id().as_u128(), a, STUDENT)),
        );
        let listed = ListStudentAnnouncements::new(
            notifications,
            InMemoryAnnouncementRepository::with_announcements(sent),
            FakeStudents,
        )
        .execute(STUDENT)
        .await
        .unwrap();
        assert_eq!(listed.len(), 50);
        assert_eq!(listed[0].announcement_id().as_u128(), 51);
        assert_eq!(
            listed[49].announcement_id().as_u128(),
            2,
            "いちばん古い 1 件は載せない"
        );
    }

    #[tokio::test]
    async fn a_student_without_announcements_is_an_empty_list() {
        let listed = ListStudentAnnouncements::new(
            InMemoryNotificationRepository::default(),
            InMemoryAnnouncementRepository::default(),
            FakeStudents,
        )
        .execute(OTHER)
        .await
        .unwrap();
        assert!(listed.is_empty());
    }

    #[tokio::test]
    async fn an_account_that_is_not_a_student_is_not_found() {
        let result = ListStudentAnnouncements::new(
            InMemoryNotificationRepository::default(),
            InMemoryAnnouncementRepository::default(),
            FakeStudents,
        )
        .execute(COACH)
        .await;
        assert!(matches!(
            result,
            Err(ListStudentAnnouncementsError::StudentNotFound)
        ));
    }

    #[tokio::test]
    async fn a_missing_announcement_is_unavailable() {
        let notifications = InMemoryNotificationRepository::with_notifications([received(
            11,
            &announcement(1, 10),
            STUDENT,
        )]);
        let result = ListStudentAnnouncements::new(
            notifications,
            InMemoryAnnouncementRepository::default(),
            FakeStudents,
        )
        .execute(STUDENT)
        .await;
        assert!(
            matches!(result, Err(ListStudentAnnouncementsError::Unavailable(_))),
            "送った記録のないお知らせは保存先の破損"
        );
    }
}
