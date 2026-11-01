//! コーチのレッスンメニュー（catalog。
//! `/coaches/me/lesson-menus`・`GET /coaches/{coach_id}/lesson-menus`・`GET /lesson-menus`）。
//! 本部の横断の一覧は作ったコーチの表示名を添える（`ProfileCoachDisplayNameLookup`）。

use application::{
    catalog::{
        CreateLessonMenu, ListCoachLessonMenus, ListLessonMenus, ListMyLessonMenus,
        UpdateLessonMenu,
    },
    profile::ProfileCoachDisplayNameLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::catalog_router(
        CreateLessonMenu::new(ports.lesson_menus(), ports.accounts(), ports.clock()),
        UpdateLessonMenu::new(ports.lesson_menus(), ports.accounts()),
        ListMyLessonMenus::new(ports.lesson_menus()),
        ListCoachLessonMenus::new(ports.lesson_menus()),
        ListLessonMenus::new(
            ports.lesson_menus(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
        ),
        MockTokenService,
    )
}
