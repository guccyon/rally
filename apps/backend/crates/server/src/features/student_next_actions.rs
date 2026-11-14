//! 生徒の次の操作（`GET /me/next-actions`。ADR 0010）。予約・通話セッション・動画・完了記録・受講権の保存先から読み、
//! コーチの表示名をアカウントの保存先から読んで添える（`ProfileCoachDisplayNameLookup`）。

use application::{profile::ProfileCoachDisplayNameLookup, scheduling::GetMyNextActions};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::student_next_actions_router(
        GetMyNextActions::new(
            ports.bookings(),
            ports.sessions(),
            ports.videos(),
            ports.completions(),
            ports.entitlements(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
