//! プッシュ通知の送り先のモック（`MockPushSender`）の検査。送った通知を送った順に記録し、複製と共有することと、
//! 標準出力に書く 1 行の形を確かめる。

use application::notification::PushSender;
use domain::{
    async_lesson::{VideoId, VideoReviewId},
    identity::AccountId,
    messaging::{MessageId, MessageSender, MessageThreadId},
    notification::PushNotification,
    scheduling::BookingId,
};
use infra::MockPushSender;

fn confirmed(booking: u128) -> PushNotification {
    PushNotification::BookingConfirmed {
        student_id: AccountId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef),
        coach_id: AccountId::from_u128(9),
        booking_id: BookingId::from_u128(booking),
    }
}

#[tokio::test]
async fn records_sent_notifications_in_order_and_shares_them_with_clones() {
    let sender = MockPushSender::new();
    let clone = sender.clone();
    assert!(sender.sent().is_empty());
    clone.send(confirmed(1)).await.unwrap();
    sender.send(confirmed(2)).await.unwrap();
    assert_eq!(sender.sent(), [confirmed(1), confirmed(2)]);
    assert_eq!(clone.sent(), sender.sent());
}

#[test]
fn writes_one_line_per_notification() {
    assert_eq!(
        MockPushSender::line(&confirmed(1)),
        "push_notification 01234567-89ab-4def-8123-456789abcdef booking_confirmed \
         00000000-0000-0000-0000-000000000001"
    );
}

#[test]
fn writes_the_video_for_a_returned_review_and_the_thread_for_a_received_message() {
    let student = AccountId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
    let coach = AccountId::from_u128(9);
    assert_eq!(
        MockPushSender::line(&PushNotification::VideoReviewReturned {
            student_id: student,
            coach_id: coach,
            video_id: VideoId::from_u128(2),
            review_id: VideoReviewId::from_u128(3),
        }),
        "push_notification 01234567-89ab-4def-8123-456789abcdef video_review_returned \
         00000000-0000-0000-0000-000000000002"
    );
    // 生徒が送ったメッセージは、受け手のコーチに届く
    assert_eq!(
        MockPushSender::line(&PushNotification::MessageReceived {
            student_id: student,
            coach_id: coach,
            thread_id: MessageThreadId::from_u128(4),
            message_id: MessageId::from_u128(5),
            sender: MessageSender::Student,
        }),
        "push_notification 00000000-0000-0000-0000-000000000009 message_received \
         00000000-0000-0000-0000-000000000004"
    );
}
