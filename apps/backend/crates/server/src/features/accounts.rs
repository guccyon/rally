//! 生徒の登録（`POST /accounts`）。

use application::identity::RegisterAccount;
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::accounts_router(
        RegisterAccount::new(ports.accounts(), ports.analytics(), ports.clock()),
        MockTokenService,
    )
}
