//! 本部が見るキャンセルされた予約の一覧（`GET /bookings/cancelled`）。コーチと生徒の表示名をアカウントの保存先から読んで
//! 添える。

use application::{
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
    scheduling::ListCancelledBookings,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::headquarters_cancelled_bookings_router(
        ListCancelledBookings::new(
            ports.bookings(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
