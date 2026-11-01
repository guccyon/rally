//! 本部によるレッスンメニューの非公開・公開に戻す操作（`POST /lesson-menus/{lesson_menu_id}/unpublish`・`/republish`）。

use application::catalog::{RepublishLessonMenu, UnpublishLessonMenu};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::lesson_menu_publication_router(
        UnpublishLessonMenu::new(ports.lesson_menus(), ports.clock()),
        RepublishLessonMenu::new(ports.lesson_menus()),
        MockTokenService,
    )
}
