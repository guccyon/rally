//! プッシュ通知の送り先のモック（`MockPushSender`）の検査。送った通知を送った順に記録し、複製と共有することと、
//! 標準出力に書く 1 行の形を確かめる。

use application::notification::PushSender;
use domain::{identity::AccountId, notification::PushNotification, scheduling::BookingId};
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
