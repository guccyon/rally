//! コーチの自分の枠の一覧（`GET /coaches/me/slots`）。スロットの保存先から枠を、予約の保存先から枠に入った予約を読む。

use application::{catalog::CatalogLessonMenuLookup, scheduling::ListCoachSlots};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::coach_slots_router(
        ListCoachSlots::new(
            ports.slots(),
            CatalogLessonMenuLookup::new(ports.lesson_menus()),
            ports.bookings(),
        ),
        MockTokenService,
    )
}
