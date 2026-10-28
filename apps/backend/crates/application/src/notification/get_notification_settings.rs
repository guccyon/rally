use domain::{identity::AccountId, notification::NotificationSettings};

use super::NotificationSettingsRepository;
use crate::Unavailable;

/// 自分の通知の受け取り設定の照会のユースケース（student-app の `getMyNotificationSettings`）。照会だけで、何も保存しない。
/// まだ変えていなければ既定（すべてオン）を返す。誰が呼べるか（役割）は境界（interface）が確かめる。
pub struct GetNotificationSettings<S> {
    settings: S,
}

impl<S> GetNotificationSettings<S>
where
    S: NotificationSettingsRepository,
{
    pub fn new(settings: S) -> Self {
        Self { settings }
    }

    /// `account_id` は認証したアカウント。
    pub async fn execute(
        &self,
        account_id: AccountId,
    ) -> Result<NotificationSettings, Unavailable> {
        self.settings.find(account_id).await
    }
}

#[cfg(test)]
mod tests {
    use domain::notification::NotificationKind;

    use super::*;
    use crate::in_memory::InMemoryNotificationSettingsRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);

    #[tokio::test]
    async fn an_account_that_has_not_changed_anything_gets_every_kind_on() {
        let repository = InMemoryNotificationSettingsRepository::default();
        let settings = GetNotificationSettings::new(repository.clone())
            .execute(STUDENT)
            .await
            .unwrap();
        assert_eq!(settings, NotificationSettings::new(STUDENT));
        assert!(repository.all().is_empty(), "照会では保存しない");
    }

    #[tokio::test]
    async fn returns_the_saved_settings() {
        let saved = NotificationSettings::restore(STUDENT, [NotificationKind::MessageReceived]);
        let settings =
            GetNotificationSettings::new(InMemoryNotificationSettingsRepository::with_settings([
                saved.clone(),
            ]))
            .execute(STUDENT)
            .await
            .unwrap();
        assert_eq!(settings, saved);
    }
}
