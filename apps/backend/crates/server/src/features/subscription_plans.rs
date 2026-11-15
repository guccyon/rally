//! 契約プランの一覧（`GET /subscription-plans`）。

use application::entitlement::ListSubscriptionPlans;
use axum::Router;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::subscription_plans_router(ListSubscriptionPlans::new(), ports.tokens())
}
