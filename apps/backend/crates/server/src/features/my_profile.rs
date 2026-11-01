//! 生徒の自分のプロフィール（`GET`・`PATCH /me/profile`）。

use application::{identity::GetAccount, profile::UpdateStudentProfile};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::my_profile_router(
        GetAccount::new(ports.accounts()),
        UpdateStudentProfile::new(ports.accounts()),
        MockTokenService,
    )
}
