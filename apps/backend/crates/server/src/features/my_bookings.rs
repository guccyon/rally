//! 生徒の自分の予約の一覧（`GET /bookings/me`）。
//! コーチの表示名と自己紹介文をアカウントの保存先から読んで添える（`ProfileCoachIntroductionLookup`）。

use application::{
    entitlement::EntitlementQuotaGate, profile::ProfileCoachIntroductionLookup,
    scheduling::ListMyBookings,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::my_bookings_router(
        ListMyBookings::new(
            ports.bookings(),
            ProfileCoachIntroductionLookup::new(ports.accounts()),
            EntitlementQuotaGate::new(ports.entitlements()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
