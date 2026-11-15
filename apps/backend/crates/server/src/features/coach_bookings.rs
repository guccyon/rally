//! コーチの自分の予約の一覧（`GET /coaches/me/bookings`）。
//! 予約した生徒の表示名をアカウントの保存先から読んで添える（`ProfileStudentDisplayNameLookup`）。予約が結び付いた
//! 通話中のセッションは通話セッションの保存先から読んで添える。

use application::{profile::ProfileStudentDisplayNameLookup, scheduling::ListCoachBookings};
use axum::Router;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::coach_bookings_router(
        ListCoachBookings::new(
            ports.bookings(),
            ports.sessions(),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        ports.tokens(),
    )
}
