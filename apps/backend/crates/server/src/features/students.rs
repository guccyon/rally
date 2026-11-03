//! 本部が見る生徒の一覧（`GET /students`）。契約種別と契約プランを受講権の保存先から、
//! 最後に予約した時刻と直近 30 日の予約の数を予約の保存先から読んで添える。コーチで絞るときは、そのコーチを予約したことがある生徒を予約の保存先から読む。

use application::{
    entitlement::EntitlementStudentContractLookup,
    identity::ListStudents,
    scheduling::{
        SchedulingStudentCoachBookingLookup, SchedulingStudentLastBookingLookup,
        SchedulingStudentRecentBookingCountLookup,
    },
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::students_router(
        ListStudents::new(
            ports.accounts(),
            EntitlementStudentContractLookup::new(ports.entitlements()),
            SchedulingStudentLastBookingLookup::new(ports.bookings()),
            SchedulingStudentCoachBookingLookup::new(ports.bookings()),
            SchedulingStudentRecentBookingCountLookup::new(ports.bookings()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
