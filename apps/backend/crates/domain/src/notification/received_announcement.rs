use std::time::SystemTime;

use super::{Announcement, AnnouncementId, AnnouncementTitle, Notification, NotificationError};
use crate::identity::AccountId;

/// 生徒 1 人が受け取った本部からのお知らせ（本部が生徒からの問い合わせで確かめるもの）。生徒ごとのお知らせ
/// （[`Notification`]。既読はここ）と、その送った記録（[`Announcement`]。題・送った時刻・取り下げはここ）の組。
///
/// 取り下げたお知らせも含む（生徒のお知らせの一覧には出ないが、届いたことと既読は残っている）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedAnnouncement {
    notification: Notification,
    announcement: Announcement,
}

impl ReceivedAnnouncement {
    /// お知らせ `notification` と送った記録 `announcement` の組。お知らせが本部からのお知らせでないか、別の送った記録を
    /// 指していれば [`NotificationError::ReceivedAnnouncementMismatch`]。
    pub fn new(
        notification: Notification,
        announcement: Announcement,
    ) -> Result<Self, NotificationError> {
        if notification.push().announcement_id() != Some(announcement.id()) {
            return Err(NotificationError::ReceivedAnnouncementMismatch);
        }
        Ok(Self {
            notification,
            announcement,
        })
    }

    /// 受け取った生徒。
    pub fn student_id(&self) -> AccountId {
        self.notification.recipient()
    }

    pub fn announcement_id(&self) -> AnnouncementId {
        self.announcement.id()
    }

    pub fn title(&self) -> &AnnouncementTitle {
        self.announcement.title()
    }

    /// 本部が送った時刻。
    pub fn sent_at(&self) -> SystemTime {
        self.announcement.sent_at()
    }

    /// 生徒が読んだか（生徒のお知らせの既読）。
    pub fn is_read(&self) -> bool {
        self.notification.is_read()
    }

    /// 本部が取り下げたか。
    pub fn is_withdrawn(&self) -> bool {
        self.announcement.is_withdrawn()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        notification::{AnnouncementBody, NotificationId, PushNotification},
        scheduling::BookingId,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const HEADQUARTERS: AccountId = AccountId::from_u128(5);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn announcement(id: u128) -> Announcement {
        Announcement::new(
            AnnouncementId::from_u128(id),
            HEADQUARTERS,
            AnnouncementTitle::new("メンテナンスのお知らせ").unwrap(),
            AnnouncementBody::new("11/3 は使えません").unwrap(),
            None,
            2,
            at(10),
        )
        .unwrap()
    }

    fn notification(push: PushNotification) -> Notification {
        Notification::new(NotificationId::from_u128(3), push, at(10))
    }

    #[test]
    fn pairs_the_notification_with_what_headquarters_sent() {
        let sent = announcement(7);
        let mut to_student = notification(sent.push_to(STUDENT));
        let received = ReceivedAnnouncement::new(to_student, sent.clone()).unwrap();
        assert_eq!(received.student_id(), STUDENT);
        assert_eq!(received.announcement_id(), AnnouncementId::from_u128(7));
        assert_eq!(received.title().as_str(), "メンテナンスのお知らせ");
        assert_eq!(received.sent_at(), at(10));
        assert!(!received.is_read(), "読むまでは未読");
        assert!(!received.is_withdrawn());

        to_student.mark_read(at(20));
        let mut withdrawn = sent;
        withdrawn.withdraw(HEADQUARTERS, at(30)).unwrap();
        let received = ReceivedAnnouncement::new(to_student, withdrawn).unwrap();
        assert!(received.is_read(), "既読は生徒のお知らせから");
        assert!(received.is_withdrawn(), "取り下げは送った記録から");
    }

    #[test]
    fn the_notification_must_point_to_the_announcement() {
        assert_eq!(
            ReceivedAnnouncement::new(
                notification(announcement(8).push_to(STUDENT)),
                announcement(7)
            ),
            Err(NotificationError::ReceivedAnnouncementMismatch),
            "別の送った記録"
        );
        assert_eq!(
            ReceivedAnnouncement::new(
                notification(PushNotification::BookingConfirmed {
                    student_id: STUDENT,
                    coach_id: AccountId::from_u128(9),
                    booking_id: BookingId::from_u128(3),
                }),
                announcement(7)
            ),
            Err(NotificationError::ReceivedAnnouncementMismatch),
            "本部からのお知らせでない"
        );
    }
}
