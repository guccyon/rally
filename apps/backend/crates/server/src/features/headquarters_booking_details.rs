//! 本部が見る予約 1 件の詳細（`GET /bookings/{booking_id}`）。コーチと生徒の表示名をアカウントの保存先から読んで添える。

use application::{
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
    scheduling::GetBooking,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::headquarters_booking_detail_router(
        GetBooking::new(
            ports.bookings(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
        ),
        MockTokenService,
    )
}
