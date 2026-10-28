use domain::{
    identity::AccountId,
    notification::{NotificationKind, NotificationSettings},
};

use super::NotificationSettingsRepository;
use crate::Unavailable;

/// 通知の受け取り設定の変更の入力。`account_id` は認証したアカウント（役割の確認は境界が行う）。`changes` は種類ごとの
/// 受け取るか（オン / オフ）で、並べていない種類は今の設定のまま。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateNotificationSettingsCommand {
    pub account_id: AccountId,
    pub changes: Vec<(NotificationKind, bool)>,
}

/// 自分の通知の受け取り設定の変更のユースケース（student-app の `updateMyNotificationSettings`）。変えたあとの設定を返す。
/// 行動イベントも境界間イベントも発火しない（通知を送るときに [`super::PushNotificationSubscriber`] が読む）。
pub struct UpdateNotificationSettings<S> {
    settings: S,
}

impl<S> UpdateNotificationSettings<S>
where
    S: NotificationSettingsRepository,
{
    pub fn new(settings: S) -> Self {
        Self { settings }
    }

    pub async fn execute(
        &self,
        command: UpdateNotificationSettingsCommand,
    ) -> Result<NotificationSettings, Unavailable> {
        let UpdateNotificationSettingsCommand {
            account_id,
            changes,
        } = command;
        self.settings
            .update(account_id, move |settings| {
                for (kind, enabled) in changes {
                    settings.set(kind, enabled);
                }
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory::InMemoryNotificationSettingsRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);

    #[tokio::test]
    async fn changes_only_the_given_kinds_and_saves_them() {
        let repository = InMemoryNotificationSettingsRepository::default();
        let use_case = UpdateNotificationSettings::new(repository.clone());

        let updated = use_case
            .execute(UpdateNotificationSettingsCommand {
                account_id: STUDENT,
                changes: vec![(NotificationKind::MessageReceived, false)],
            })
            .await
            .unwrap();
        assert_eq!(
            updated,
            NotificationSettings::restore(STUDENT, [NotificationKind::MessageReceived])
        );

        // 並べていない種類（メッセージ）は今の設定のまま
        let updated = use_case
            .execute(UpdateNotificationSettingsCommand {
                account_id: STUDENT,
                changes: vec![
                    (NotificationKind::BookingConfirmed, false),
                    (NotificationKind::VideoReviewReturned, true),
                ],
            })
            .await
            .unwrap();
        assert_eq!(
            updated,
            NotificationSettings::restore(
                STUDENT,
                [
                    NotificationKind::BookingConfirmed,
                    NotificationKind::MessageReceived
                ]
            )
        );
        assert_eq!(repository.all(), [updated], "保存したのは変えた設定");
    }
}
