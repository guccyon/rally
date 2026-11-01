//! 自分の通知の受け取り設定（`GET`・`PUT /notification-settings/me`）。

use application::notification::{GetNotificationSettings, UpdateNotificationSettings};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::notification_settings_router(
        GetNotificationSettings::new(ports.notification_settings()),
        UpdateNotificationSettings::new(ports.notification_settings()),
        MockTokenService,
    )
}
