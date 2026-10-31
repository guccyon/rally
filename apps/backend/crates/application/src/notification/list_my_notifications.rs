use domain::identity::AccountId;

use super::{AnnouncementRepository, MyNotification, NotificationRepository};
use crate::Unavailable;

/// 自分のお知らせの一覧のユースケース（student-app と admin-web の `listMyNotifications`。生徒もコーチも、自分が受け取る
/// お知らせだけ）。照会だけで、何も保存しない。
/// 作った時刻の新しい順で、受け取り設定でオフの種類のお知らせも含む。本部からのお知らせには送った記録を添える
/// （[`MyNotification`]）。誰が呼べるか（役割）は境界（interface）が確かめる。行動イベントは発火しない。
pub struct ListMyNotifications<N, A> {
    notifications: N,
    announcements: A,
}

impl<N, A> ListMyNotifications<N, A>
where
    N: NotificationRepository,
    A: AnnouncementRepository,
{
    pub fn new(notifications: N, announcements: A) -> Self {
        Self {
            notifications,
            announcements,
        }
    }

    /// `recipient` は認証したアカウント。
    pub async fn execute(&self, recipient: AccountId) -> Result<Vec<MyNotification>, Unavailable> {
        let notifications = self.notifications.find_by_recipient(recipient).await?;
        MyNotification::attach(notifications, &self.announcements).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::VideoId,
        messaging::{MessageId, MessageSender, MessageThreadId},
        notification::{
            Announcement, AnnouncementBody, AnnouncementId, AnnouncementTitle, Notification,
            NotificationId, PushNotification,
        },
        scheduling::BookingId,
    };

    use super::*;
    use crate::in_memory::{InMemoryAnnouncementRepository, InMemoryNotificationRepository};

    /// 生徒・コーチの `recipient` のお知らせ（本部からのお知らせはない）の中身だけ
    async fn listed_of(
        notifications: InMemoryNotificationRepository,
        recipient: AccountId,
    ) -> Vec<Notification> {
        ListMyNotifications::new(notifications, InMemoryAnnouncementRepository::default())
            .execute(recipient)
            .await
            .unwrap()
            .into_iter()
            .map(|mine| {
                assert_eq!(mine.announcement, None);
                mine.notification
            })
            .collect()
    }

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(9);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn confirmed(id: u128, student_id: AccountId, minute: u64) -> Notification {
        Notification::new(
            NotificationId::from_u128(id),
            PushNotification::BookingConfirmed {
                student_id,
                coach_id: COACH,
                booking_id: BookingId::from_u128(id),
            },
            at(minute),
        )
    }

    #[tokio::test]
    async fn lists_only_my_notifications_newest_first() {
        let (old, new, same_time, others) = (
            confirmed(1, STUDENT, 10),
            confirmed(2, STUDENT, 20),
            confirmed(3, STUDENT, 20),
            confirmed(4, OTHER, 30),
        );
        let notifications =
            InMemoryNotificationRepository::with_notifications([old, new, same_time, others]);
        let listed = listed_of(notifications.clone(), STUDENT).await;
        assert_eq!(
            listed,
            [same_time, new, old],
            "新しい順。同じ時刻は後に残したものが先。ほかの生徒の分は出さない"
        );
        assert_eq!(notifications.all().len(), 4, "照会では何も変えない");
    }

    #[tokio::test]
    async fn a_coach_lists_only_the_notifications_sent_to_the_coach() {
        let notification = |id: u128, push, minute| {
            Notification::new(NotificationId::from_u128(id), push, at(minute))
        };
        let received = notification(
            1,
            PushNotification::BookingReceived {
                student_id: STUDENT,
                coach_id: COACH,
                booking_id: BookingId::from_u128(1),
            },
            10,
        );
        let requested = notification(
            2,
            PushNotification::VideoReviewRequested {
                student_id: STUDENT,
                coach_id: COACH,
                video_id: VideoId::from_u128(2),
            },
            20,
        );
        let message = |id: u128, sender, minute| {
            notification(
                id,
                PushNotification::MessageReceived {
                    student_id: STUDENT,
                    coach_id: COACH,
                    thread_id: MessageThreadId::from_u128(3),
                    message_id: MessageId::from_u128(id),
                    sender,
                },
                minute,
            )
        };
        let from_student = message(3, MessageSender::Student, 30);
        let notifications = InMemoryNotificationRepository::with_notifications([
            confirmed(4, STUDENT, 5),
            received,
            requested,
            from_student,
            message(5, MessageSender::Coach, 40),
        ]);
        let listed = listed_of(notifications, COACH).await;
        assert_eq!(
            listed,
            [from_student, requested, received],
            "新しい予約・添削の依頼・生徒からのメッセージだけ。生徒が受け取る分は出さない"
        );
    }

    #[tokio::test]
    async fn no_notifications_is_an_empty_list() {
        let listed = listed_of(InMemoryNotificationRepository::default(), STUDENT).await;
        assert!(listed.is_empty());
    }

    fn announcement(id: u128) -> Announcement {
        Announcement::new(
            AnnouncementId::from_u128(id),
            AccountId::from_u128(5),
            AnnouncementTitle::new("メンテナンスのお知らせ").unwrap(),
            AnnouncementBody::new("11/3 は使えません").unwrap(),
            None,
            2,
            at(30),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn an_announcement_comes_with_what_headquarters_sent() {
        let sent = announcement(7);
        let to_student =
            Notification::new(NotificationId::from_u128(2), sent.push_to(STUDENT), at(30));
        let notifications = InMemoryNotificationRepository::with_notifications([
            confirmed(1, STUDENT, 10),
            to_student,
            Notification::new(NotificationId::from_u128(3), sent.push_to(OTHER), at(30)),
        ]);
        let announcements =
            InMemoryAnnouncementRepository::with_announcements([announcement(8), sent.clone()]);
        let listed = ListMyNotifications::new(notifications, announcements)
            .execute(STUDENT)
            .await
            .unwrap();
        assert_eq!(
            listed,
            [
                MyNotification {
                    notification: to_student,
                    announcement: Some(sent),
                },
                MyNotification {
                    notification: confirmed(1, STUDENT, 10),
                    announcement: None,
                },
            ]
        );
    }

    #[tokio::test]
    async fn a_missing_announcement_is_unavailable() {
        let notifications =
            InMemoryNotificationRepository::with_notifications([Notification::new(
                NotificationId::from_u128(2),
                announcement(7).push_to(STUDENT),
                at(30),
            )]);
        let result =
            ListMyNotifications::new(notifications, InMemoryAnnouncementRepository::default())
                .execute(STUDENT)
                .await;
        assert!(result.is_err(), "送った記録のないお知らせは保存先の破損");
    }
}
