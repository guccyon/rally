use crate::{
    async_lesson::{VideoId, VideoReviewId},
    identity::AccountId,
    messaging::{MessageId, MessageSender, MessageThreadId},
    scheduling::BookingId,
};

/// 1 件のプッシュ通知。変種は通知のきっかけ（境界間イベント）ごとで、受け取る人は [`PushNotification::recipient`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushNotification {
    /// 予約が確定した（`BookingConfirmed`）ことを生徒に知らせる
    BookingConfirmed {
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
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
}

impl PushNotification {
    /// 通知を受け取るアカウント。
    pub const fn recipient(&self) -> AccountId {
        match *self {
            PushNotification::BookingConfirmed { student_id, .. }
            | PushNotification::VideoReviewReturned { student_id, .. } => student_id,
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
    }
}
