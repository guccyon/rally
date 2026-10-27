//! プッシュ通知の送り先の基盤（SNS / Pusher 相当）のモック（Gen1 の外部連携はすべてモック）。

use std::sync::{Arc, Mutex};

use application::{
    Unavailable, identity::account_id_uuid, notification::PushSender, scheduling::booking_id_uuid,
};
use domain::notification::PushNotification;

/// [`PushSender`] のモック。実際の端末には届けず、送った通知を記録して（[`MockPushSender::sent`]）、
/// `push_notification <受け取るアカウントの ID> <種類> <対象の ID>` の 1 行で標準出力に書く。
/// 記録はプロセスのメモリにだけ持ち、複製は同じ記録を共有する。
#[derive(Debug, Clone, Default)]
pub struct MockPushSender {
    sent: Arc<Mutex<Vec<PushNotification>>>,
}

impl MockPushSender {
    pub fn new() -> Self {
        Self::default()
    }

    /// 送った通知（送った順）。
    pub fn sent(&self) -> Vec<PushNotification> {
        self.sent.lock().unwrap().clone()
    }

    /// 標準出力に書く 1 行。
    pub fn line(notification: &PushNotification) -> String {
        let recipient = account_id_uuid::format(notification.recipient());
        match *notification {
            PushNotification::BookingConfirmed { booking_id, .. } => format!(
                "push_notification {recipient} booking_confirmed {}",
                booking_id_uuid::format(booking_id)
            ),
        }
    }
}

impl PushSender for MockPushSender {
    async fn send(&self, notification: PushNotification) -> Result<(), Unavailable> {
        println!("{}", Self::line(&notification));
        self.sent.lock().unwrap().push(notification);
        Ok(())
    }
}
