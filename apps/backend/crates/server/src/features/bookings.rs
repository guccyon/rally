//! 生徒の予約の作成・確定・キャンセル（`POST /bookings`・`/bookings/{booking_id}/confirm`・`/cancel`）。確定は
//! entitlement に通話の枠が残っているかを問い合わせる（`EntitlementQuotaGate`）。体験予約は、月額契約・完了したレッスン・
//! 送った動画で体験を使い終えたかを entitlement に問い合わせる（`EntitlementTrialUsageLookup`。ADR 0014）。返す予約に添えるコーチの表示名は
//! アカウントの保存先から読む。

use application::{
    catalog::CatalogLessonMenuLookup,
    entitlement::{EntitlementQuotaGate, EntitlementTrialUsageLookup},
    profile::ProfileCoachDisplayNameLookup,
    scheduling::{CancelBooking, ConfirmBooking, CreateBooking, DescribeBooking},
};
use axum::Router;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::bookings_router(
        CreateBooking::new(
            ports.bookings(),
            CatalogLessonMenuLookup::new(ports.lesson_menus()),
            EntitlementTrialUsageLookup::new(
                ports.entitlements(),
                ports.completions(),
                ports.videos(),
            ),
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
        DescribeBooking::new(
            ports.bookings(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
        ),
        ports.tokens(),
    )
}
