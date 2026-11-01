//! ログイン中の本人（`GET /me`）とコーチの自己紹介文の変更（`PUT /me/profile`）。

use application::{
    identity::GetAccount,
    profile::{GetCoachBio, UpdateCoachProfile},
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::me_router(
        GetAccount::new(ports.accounts()),
        GetCoachBio::new(ports.accounts()),
        UpdateCoachProfile::new(ports.accounts()),
        MockTokenService,
    )
}
