use domain::{identity::AccountId, notification::Notification};

use super::NotificationRepository;
use crate::{Unavailable, clock::Clock};

/// 自分のお知らせをすべて既読にするユースケース（student-app と admin-web の `markMyNotificationsRead`）。今の時刻で既読にし
/// （すでに読んだものは最初に読んだ時刻のまま。何度呼んでもよい）、既読にしたあとの一覧（[`super::ListMyNotifications`]
/// と同じ並び）を返す。誰が呼べるか（役割）は境界（interface）が確かめる。行動イベントは発火しない。
pub struct MarkMyNotificationsRead<N, C> {
    notifications: N,
    clock: C,
}

impl<N, C> MarkMyNotificationsRead<N, C>
where
    N: NotificationRepository,
    C: Clock,
{
    pub fn new(notifications: N, clock: C) -> Self {
        Self {
            notifications,
            clock,
        }
    }

    /// `recipient` は認証したアカウント。
    pub async fn execute(&self, recipient: AccountId) -> Result<Vec<Notification>, Unavailable> {
        self.notifications
            .mark_all_read(recipient, self.clock.now())
            .await?;
        self.notifications.find_by_recipient(recipient).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::VideoId,
        notification::{NotificationId, PushNotification},
        scheduling::BookingId,
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryNotificationRepository};

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
    async fn marks_every_notification_of_mine_read_and_keeps_the_first_time() {
        let mut already_read = confirmed(1, STUDENT, 10);
        already_read.mark_read(at(15));
        let notifications = InMemoryNotificationRepository::with_notifications([
            already_read,
            confirmed(2, STUDENT, 20),
            confirmed(3, OTHER, 20),
        ]);

        let listed = MarkMyNotificationsRead::new(notifications.clone(), FixedClock(at(30)))
            .execute(STUDENT)
            .await
            .unwrap();
        let read_at: Vec<_> = listed.iter().map(Notification::read_at).collect();
        assert_eq!(
            read_at,
            [Some(at(30)), Some(at(15))],
            "新しい順。読んだものは最初に読んだ時刻のまま"
        );
        assert_eq!(
            listed,
            notifications.all()[..2]
                .iter()
                .rev()
                .copied()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            notifications.all()[2].read_at(),
            None,
            "ほかの生徒のお知らせは変えない"
        );

        // 何度呼んでもよい
        let again = MarkMyNotificationsRead::new(notifications, FixedClock(at(40)))
            .execute(STUDENT)
            .await
            .unwrap();
        assert_eq!(again, listed);
    }

    #[tokio::test]
    async fn a_coach_marks_only_the_notifications_sent_to_the_coach_read() {
        let requested = Notification::new(
            NotificationId::from_u128(2),
            PushNotification::VideoReviewRequested {
                student_id: STUDENT,
                coach_id: COACH,
                video_id: VideoId::from_u128(2),
            },
            at(20),
        );
        let notifications = InMemoryNotificationRepository::with_notifications([
            confirmed(1, STUDENT, 10),
            requested,
        ]);

        let listed = MarkMyNotificationsRead::new(notifications.clone(), FixedClock(at(30)))
            .execute(COACH)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id(), requested.id());
        assert_eq!(listed[0].read_at(), Some(at(30)));
        assert_eq!(
            notifications.all()[0].read_at(),
            None,
            "生徒が受け取るお知らせは変えない"
        );
    }

    #[tokio::test]
    async fn no_notifications_is_an_empty_list() {
        let listed = MarkMyNotificationsRead::new(
            InMemoryNotificationRepository::default(),
            FixedClock(at(30)),
        )
        .execute(STUDENT)
        .await
        .unwrap();
        assert!(listed.is_empty());
    }
}
