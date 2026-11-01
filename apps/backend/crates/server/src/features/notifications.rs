//! 生徒とコーチのお知らせ（`GET /notifications/me`・`POST /notifications/me/read`）。本部からのお知らせは、送った記録の
//! 保存先から読む。

use application::notification::{ListMyNotifications, MarkMyNotificationsRead};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::notifications_router(
        ListMyNotifications::new(ports.notifications(), ports.announcements()),
        MarkMyNotificationsRead::new(ports.notifications(), ports.announcements(), ports.clock()),
        MockTokenService,
    )
}
