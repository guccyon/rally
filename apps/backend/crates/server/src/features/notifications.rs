//! 生徒とコーチのお知らせ（`GET /notifications/me`・`POST /notifications/me/read`）。

use application::notification::{ListMyNotifications, MarkMyNotificationsRead};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::notifications_router(
        ListMyNotifications::new(ports.notifications()),
        MarkMyNotificationsRead::new(ports.notifications(), ports.clock()),
        MockTokenService,
    )
}
