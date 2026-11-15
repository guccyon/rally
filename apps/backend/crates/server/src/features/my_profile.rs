//! 生徒の自分のプロフィール（`GET`・`PATCH /me/profile`）。

use application::{identity::GetAccount, profile::UpdateStudentProfile};
use axum::Router;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::my_profile_router(
        GetAccount::new(ports.accounts()),
        UpdateStudentProfile::new(ports.accounts()),
        ports.tokens(),
    )
}
