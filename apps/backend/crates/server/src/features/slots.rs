//! スロットの公開と検索（`POST`・`GET /slots`）。
//! 検索はコーチの表示名と自己紹介文をアカウントの保存先から読んで添える（`ProfileCoachIntroductionLookup`）。

use application::{
    catalog::CatalogLessonMenuLookup,
    profile::ProfileCoachIntroductionLookup,
    scheduling::{PublishSlot, SearchSlots},
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::slots_router(
        PublishSlot::new(
            ports.slots(),
            CatalogLessonMenuLookup::new(ports.lesson_menus()),
            ports.analytics(),
            ports.clock(),
        ),
        SearchSlots::new(
            ports.slots(),
            CatalogLessonMenuLookup::new(ports.lesson_menus()),
            ProfileCoachIntroductionLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
