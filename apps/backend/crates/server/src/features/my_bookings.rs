//! 生徒の自分の予約の一覧（`GET /bookings/me`）。
//! コーチの表示名と自己紹介文をアカウントの保存先から読んで添える（`ProfileCoachIntroductionLookup`）。体験予約できるか
//! （`trial_available`）は、予約の作成と同じ体験の使用状況の照会（`EntitlementTrialUsageLookup`）で決める。

use application::{
    entitlement::{EntitlementQuotaGate, EntitlementTrialUsageLookup},
    profile::ProfileCoachIntroductionLookup,
    scheduling::ListMyBookings,
};
use axum::Router;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::my_bookings_router(
        ListMyBookings::new(
            ports.bookings(),
            ProfileCoachIntroductionLookup::new(ports.accounts()),
            EntitlementQuotaGate::new(ports.entitlements()),
            EntitlementTrialUsageLookup::new(
                ports.entitlements(),
                ports.completions(),
                ports.videos(),
            ),
            ports.clock(),
        ),
        ports.tokens(),
    )
}
