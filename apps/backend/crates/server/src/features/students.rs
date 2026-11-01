//! 本部が見る生徒の一覧（`GET /students`）。契約種別と契約プランを受講権の保存先から、
//! 最後に予約した時刻を予約の保存先から読んで添える。

use application::{
    entitlement::EntitlementStudentContractLookup, identity::ListStudents,
    scheduling::SchedulingStudentLastBookingLookup,
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
        ),
        MockTokenService,
    )
}
