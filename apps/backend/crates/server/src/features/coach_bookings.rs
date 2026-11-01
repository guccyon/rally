//! コーチの自分の予約の一覧（`GET /coaches/me/bookings`）。
//! 予約した生徒の表示名をアカウントの保存先から読んで添える（`ProfileStudentDisplayNameLookup`）。

use application::{profile::ProfileStudentDisplayNameLookup, scheduling::ListCoachBookings};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::coach_bookings_router(
        ListCoachBookings::new(
            ports.bookings(),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
        ),
        MockTokenService,
    )
}
