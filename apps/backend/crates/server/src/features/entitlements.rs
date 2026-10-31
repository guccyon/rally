//! 受講権の照会（`GET /entitlements/me`）。

use application::entitlement::GetEntitlement;
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::entitlements_router(
        GetEntitlement::new(ports.entitlements(), ports.clock()),
        MockTokenService,
    )
}
