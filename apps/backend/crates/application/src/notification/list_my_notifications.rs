use domain::{identity::AccountId, notification::Notification};

use super::NotificationRepository;
use crate::Unavailable;

/// 自分のお知らせの一覧のユースケース（student-app の `listMyNotifications`）。照会だけで、何も保存しない。
/// 作った時刻の新しい順で、受け取り設定でオフの種類のお知らせも含む。誰が呼べるか（役割）は境界（interface）が確かめる。
/// 行動イベントは発火しない。
pub struct ListMyNotifications<N> {
    notifications: N,
}

impl<N> ListMyNotifications<N>
where
    N: NotificationRepository,
{
    pub fn new(notifications: N) -> Self {
        Self { notifications }
    }

    /// `recipient` は認証したアカウント。
    pub async fn execute(&self, recipient: AccountId) -> Result<Vec<Notification>, Unavailable> {
        self.notifications.find_by_recipient(recipient).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        notification::{NotificationId, PushNotification},
        scheduling::BookingId,
    };

    use super::*;
    use crate::in_memory::InMemoryNotificationRepository;

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
        let listed = ListMyNotifications::new(notifications.clone())
            .execute(STUDENT)
            .await
            .unwrap();
        assert_eq!(
            listed,
            [same_time, new, old],
            "新しい順。同じ時刻は後に残したものが先。ほかの生徒の分は出さない"
        );
        assert_eq!(notifications.all().len(), 4, "照会では何も変えない");
    }

    #[tokio::test]
    async fn no_notifications_is_an_empty_list() {
        let listed = ListMyNotifications::new(InMemoryNotificationRepository::default())
            .execute(STUDENT)
            .await
            .unwrap();
        assert!(listed.is_empty());
    }
}
