use super::{AnnouncementId, NotificationKind};
use crate::{
    async_lesson::{VideoId, VideoReviewId},
    identity::AccountId,
    messaging::{MessageId, MessageSender, MessageThreadId},
    scheduling::BookingId,
};

/// 1 件のプッシュ通知。変種は通知のきっかけ（境界間イベントか、本部からのお知らせ）と受け取る側ごとで、受け取る人は
/// [`PushNotification::recipient`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PushNotification {
    /// 予約が確定した（`BookingConfirmed`）ことを生徒に知らせる
    BookingConfirmed {
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
    },
    /// 予約が確定した（`BookingConfirmed`）ことを、予約を受けたコーチに新しい予約として知らせる
    BookingReceived {
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
    },
    /// 生徒が動画を添削依頼として送った（`VideoReviewRequested`）ことを、依頼を受けたコーチに知らせる
    VideoReviewRequested {
        student_id: AccountId,
        coach_id: AccountId,
        video_id: VideoId,
    },
    /// 動画の添削が返却された（`VideoReviewReturned`）ことを、動画を送った生徒に知らせる
    VideoReviewReturned {
        student_id: AccountId,
        coach_id: AccountId,
        video_id: VideoId,
        review_id: VideoReviewId,
    },
    /// チャットのメッセージが届いた（`MessageSent`）ことを、受け手（送り手でない側の当事者）に知らせる
    MessageReceived {
        student_id: AccountId,
        coach_id: AccountId,
        thread_id: MessageThreadId,
        message_id: MessageId,
        /// 送り手がスレッドのどちらの当事者か
        sender: MessageSender,
    },
    /// 本部からのお知らせ（[`super::Announcement`]）を、対象の生徒に知らせる
    Announcement {
        student_id: AccountId,
        announcement_id: AnnouncementId,
    },
}

impl PushNotification {
    /// 通知を受け取るアカウント。
    pub const fn recipient(&self) -> AccountId {
        match *self {
            PushNotification::BookingConfirmed { student_id, .. }
            | PushNotification::VideoReviewReturned { student_id, .. }
            | PushNotification::Announcement { student_id, .. } => student_id,
            PushNotification::BookingReceived { coach_id, .. }
            | PushNotification::VideoReviewRequested { coach_id, .. } => coach_id,
            PushNotification::MessageReceived {
                student_id,
                coach_id,
                sender,
                ..
            } => match sender {
                MessageSender::Student => coach_id,
                MessageSender::Coach => student_id,
            },
        }
    }

    /// 通知の種類（受け取り設定でオン / オフを決める単位）。予約の確定は生徒への知らせもコーチへの知らせも同じ種類。
    pub const fn kind(&self) -> NotificationKind {
        match *self {
            PushNotification::BookingConfirmed { .. }
            | PushNotification::BookingReceived { .. } => NotificationKind::BookingConfirmed,
            PushNotification::VideoReviewRequested { .. } => NotificationKind::VideoReviewRequested,
            PushNotification::VideoReviewReturned { .. } => NotificationKind::VideoReviewReturned,
            PushNotification::MessageReceived { .. } => NotificationKind::MessageReceived,
            PushNotification::Announcement { .. } => NotificationKind::Announcement,
        }
    }

    /// 本部からのお知らせの通知なら、そのお知らせの ID。
    pub const fn announcement_id(&self) -> Option<AnnouncementId> {
        match *self {
            PushNotification::Announcement {
                announcement_id, ..
            } => Some(announcement_id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(9);

    #[test]
    fn a_confirmed_booking_goes_to_the_student() {
        let notification = PushNotification::BookingConfirmed {
            student_id: STUDENT,
            coach_id: COACH,
            booking_id: BookingId::from_u128(3),
        };
        assert_eq!(notification.recipient(), STUDENT);
        assert_eq!(notification.kind(), NotificationKind::BookingConfirmed);
    }

    #[test]
    fn a_received_booking_goes_to_the_coach() {
        let notification = PushNotification::BookingReceived {
            student_id: STUDENT,
            coach_id: COACH,
            booking_id: BookingId::from_u128(3),
        };
        assert_eq!(notification.recipient(), COACH);
        assert_eq!(
            notification.kind(),
            NotificationKind::BookingConfirmed,
            "コーチへの知らせも予約の確定の種類"
        );
    }

    #[test]
    fn a_requested_review_goes_to_the_coach() {
        let notification = PushNotification::VideoReviewRequested {
            student_id: STUDENT,
            coach_id: COACH,
            video_id: VideoId::from_u128(3),
        };
        assert_eq!(notification.recipient(), COACH);
        assert_eq!(notification.kind(), NotificationKind::VideoReviewRequested);
    }

    #[test]
    fn a_returned_review_goes_to_the_student() {
        let notification = PushNotification::VideoReviewReturned {
            student_id: STUDENT,
            coach_id: COACH,
            video_id: VideoId::from_u128(3),
            review_id: VideoReviewId::from_u128(4),
        };
        assert_eq!(notification.recipient(), STUDENT);
        assert_eq!(notification.kind(), NotificationKind::VideoReviewReturned);
    }

    #[test]
    fn a_message_goes_to_the_party_who_did_not_send_it() {
        let received = |sender| PushNotification::MessageReceived {
            student_id: STUDENT,
            coach_id: COACH,
            thread_id: MessageThreadId::from_u128(3),
            message_id: MessageId::from_u128(4),
            sender,
        };
        assert_eq!(received(MessageSender::Student).recipient(), COACH);
        assert_eq!(received(MessageSender::Coach).recipient(), STUDENT);
        assert_eq!(
            received(MessageSender::Coach).kind(),
            NotificationKind::MessageReceived
        );
    }

    #[test]
    fn an_announcement_goes_to_the_student() {
        let notification = PushNotification::Announcement {
            student_id: STUDENT,
            announcement_id: AnnouncementId::from_u128(3),
        };
        assert_eq!(notification.recipient(), STUDENT);
        assert_eq!(notification.kind(), NotificationKind::Announcement);
        assert_eq!(
            notification.announcement_id(),
            Some(AnnouncementId::from_u128(3))
        );
    }

    #[test]
    fn only_an_announcement_has_an_announcement_id() {
        let confirmed = PushNotification::BookingConfirmed {
            student_id: STUDENT,
            coach_id: COACH,
            booking_id: BookingId::from_u128(3),
        };
        assert_eq!(confirmed.announcement_id(), None);
    }
}
