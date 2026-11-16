//! 生徒のログイン（`POST /login-codes`・`POST /sessions`）。

use application::identity::{CreateSession, RequestLoginCode};
use axum::Router;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::student_login_router(
        RequestLoginCode::new(ports.accounts(), ports.login_code_mailer(), ports.clock()),
        CreateSession::new(ports.accounts(), ports.clock()),
        ports.tokens(),
    )
}
