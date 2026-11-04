//! 本部が見る予約の一覧（`GET /bookings`）。コーチと生徒の表示名をアカウントの保存先から読んで添え、競技で絞るときは
//! コーチのアカウントも同じ保存先から読む。

use application::{
    profile::{
        ProfileCoachAccountLookup, ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup,
    },
    scheduling::ListBookings,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::headquarters_bookings_router(
        ListBookings::new(
            ports.bookings(),
            ProfileCoachAccountLookup::new(ports.accounts()),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
