//! 本部によるレッスンメニューの非公開・公開に戻す操作と、その記録の照会（`POST /lesson-menus/{lesson_menu_id}/unpublish`・
//! `/republish`・`GET /lesson-menus/{lesson_menu_id}/publication-changes`）。

use application::catalog::{
    ListLessonMenuPublicationChanges, RepublishLessonMenu, UnpublishLessonMenu,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::lesson_menu_publication_router(
        UnpublishLessonMenu::new(ports.lesson_menus(), ports.clock()),
        RepublishLessonMenu::new(ports.lesson_menus(), ports.clock()),
        ListLessonMenuPublicationChanges::new(ports.lesson_menus()),
        MockTokenService,
    )
}
