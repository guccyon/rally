//! 通知の受け取り設定の保存先（Postgres の `notification.notification_settings`）。

use application::{
    Unavailable, identity::account_id_uuid, notification::NotificationSettingsRepository,
};
use domain::{
    identity::AccountId,
    notification::{NotificationKind, NotificationSettings},
};
use sqlx::{PgExecutor, PgPool};

use crate::{
    notification_values::{notification_kind_from_value, notification_kind_value},
    pg_support::lock,
};

/// [`NotificationSettingsRepository`] の Postgres 実装。
///
/// アカウント × 種類で 1 行（`enabled`）。行のない種類は既定のオン。変更はトランザクションの中でアカウントごとの
/// advisory lock を取ってから、読み取り・変更・保存（すべての種類の行の upsert）を行う（行がまだないアカウントでも
/// 並行な変更が互いを上書きしないように）。
#[derive(Debug, Clone)]
pub struct PgNotificationSettingsRepository {
    pool: PgPool,
}

impl PgNotificationSettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl NotificationSettingsRepository for PgNotificationSettingsRepository {
    async fn find(&self, account_id: AccountId) -> Result<NotificationSettings, Unavailable> {
        settings_of(&self.pool, account_id).await
    }

    async fn update<F>(
        &self,
        account_id: AccountId,
        update: F,
    ) -> Result<NotificationSettings, Unavailable>
    where
        F: FnOnce(&mut NotificationSettings) + Send,
    {
        let account = account_id_uuid::to_uuid(account_id);
        let mut tx = self.pool.begin().await.map_err(Unavailable::new)?;
        lock(&mut tx, &format!("notification_settings:{account}"))
            .await
            .map_err(Unavailable::new)?;
        let mut settings = settings_of(&mut *tx, account_id).await?;
        update(&mut settings);
        let (kinds, enabled): (Vec<&str>, Vec<bool>) = NotificationKind::ALL
            .into_iter()
            .map(|kind| (notification_kind_value(kind), settings.is_enabled(kind)))
            .unzip();
        sqlx::query(
            "INSERT INTO notification.notification_settings (account_id, kind, enabled)
             SELECT $1, kind, enabled FROM UNNEST($2::text[], $3::boolean[]) AS t (kind, enabled)
             ON CONFLICT ON CONSTRAINT notification_settings_pkey
             DO UPDATE SET enabled = EXCLUDED.enabled",
        )
        .bind(account)
        .bind(&kinds)
        .bind(&enabled)
        .execute(&mut *tx)
        .await
        .map_err(Unavailable::new)?;
        tx.commit().await.map_err(Unavailable::new)?;
        Ok(settings)
    }
}

/// `account_id` の受け取り設定（オフの行の種類だけを覚える）。
async fn settings_of<'e>(
    executor: impl PgExecutor<'e>,
    account_id: AccountId,
) -> Result<NotificationSettings, Unavailable> {
    let kinds: Vec<String> = sqlx::query_scalar(
        "SELECT kind FROM notification.notification_settings
         WHERE account_id = $1 AND NOT enabled",
    )
    .bind(account_id_uuid::to_uuid(account_id))
    .fetch_all(executor)
    .await
    .map_err(Unavailable::new)?;
    let disabled = kinds
        .iter()
        .map(|value| {
            notification_kind_from_value(value)
                .ok_or_else(|| Unavailable::new(format!("未知の kind: {value}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NotificationSettings::restore(account_id, disabled))
}
