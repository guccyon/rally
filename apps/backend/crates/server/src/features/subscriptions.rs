//! 月額契約の開始（`POST /subscriptions`）。

use application::entitlement::StartSubscription;
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::subscriptions_router(
        StartSubscription::new(
            ports.entitlements(),
            ports.analytics(),
            ports.events(),
            ports.clock(),
        ),
        MockTokenService,
    )
}
