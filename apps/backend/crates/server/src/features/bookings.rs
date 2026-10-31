//! 生徒の予約の作成・確定・キャンセル（`POST /bookings`・`/bookings/{booking_id}/confirm`・`/cancel`）。確定は
//! entitlement に通話の枠が残っているかを問い合わせる（`EntitlementQuotaGate`）。

use application::{
    catalog::CatalogLessonMenuLookup,
    entitlement::EntitlementQuotaGate,
    scheduling::{CancelBooking, ConfirmBooking, CreateBooking},
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::bookings_router(
        CreateBooking::new(
            ports.bookings(),
            CatalogLessonMenuLookup::new(ports.lesson_menus()),
            ports.analytics(),
            ports.clock(),
        ),
        ConfirmBooking::new(
            ports.bookings(),
            EntitlementQuotaGate::new(ports.entitlements()),
            ports.analytics(),
            ports.events(),
            ports.clock(),
        ),
        CancelBooking::new(
            ports.bookings(),
            ports.analytics(),
            ports.events(),
            ports.clock(),
        ),
        MockTokenService,
    )
}
