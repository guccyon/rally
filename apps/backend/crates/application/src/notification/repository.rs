use std::future::Future;

use domain::{identity::AccountId, notification::NotificationSettings};

use crate::Unavailable;

/// 通知の受け取り設定の保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryNotificationSettingsRepository`] と、infra の Postgres
/// （`notification.notification_settings`）。
///
/// 保存していないアカウントは既定の受け取り設定（domain の [`NotificationSettings::new`]。すべてオン）として読む。
pub trait NotificationSettingsRepository: Send + Sync {
    /// アカウント `account_id` の受け取り設定。保存していなければ既定（すべてオン）。
    fn find(
        &self,
        account_id: AccountId,
    ) -> impl Future<Output = Result<NotificationSettings, Unavailable>> + Send;

    /// アカウント `account_id` の受け取り設定を読み、`update` に渡してから保存して返す。同じアカウントの読み取りから
    /// 保存までは直列にする（並行な変更が互いを上書きして消さないように）。
    fn update<F>(
        &self,
        account_id: AccountId,
        update: F,
    ) -> impl Future<Output = Result<NotificationSettings, Unavailable>> + Send
    where
        F: FnOnce(&mut NotificationSettings) + Send;
}
