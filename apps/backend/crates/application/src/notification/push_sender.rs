use std::future::Future;

use domain::notification::PushNotification;

use crate::Unavailable;

/// プッシュ通知の送り先の基盤（SNS / Pusher 相当）のポート。送り先の端末は受け取るアカウント
/// （[`PushNotification::recipient`]）で指し、端末の登録は基盤が持つ。
///
/// Gen1 の実装は infra のモック（`MockPushSender`。送った通知を記録し、標準出力に書く）と、テスト用の
/// [`crate::in_memory::RecordingPushSender`]。
pub trait PushSender: Send + Sync {
    /// 通知 `notification` を送る。基盤に渡せなければ障害を返す。
    fn send(
        &self,
        notification: PushNotification,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;
}
