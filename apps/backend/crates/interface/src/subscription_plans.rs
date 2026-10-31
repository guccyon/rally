//! `/subscription-plans`。student-app の `GET /subscription-plans`（`listSubscriptionPlans`）。生徒だけが呼べる。

use std::sync::Arc;

use application::{entitlement::ListSubscriptionPlans, identity::TokenVerifier};
use axum::{Json, Router, extract::State, routing::get};
use domain::identity::Role;
use serde::Serialize;

use crate::{
    auth::{TokenVerification, require_role},
    contract::subscription_plan_to_contract,
};

/// `/subscription-plans` のハンドラーが使うユースケース。
pub struct SubscriptionPlansState {
    pub list_subscription_plans: ListSubscriptionPlans,
}

/// student-app の `SubscriptionPlanList`。
#[derive(Serialize)]
pub struct SubscriptionPlanListBody {
    plans: Vec<SubscriptionPlanDetailBody>,
}

/// student-app の `SubscriptionPlanDetail`。`live_lessons_per_month` はビデオ通話の暦月の上限。
#[derive(Serialize)]
pub struct SubscriptionPlanDetailBody {
    plan: &'static str,
    live_lessons_per_month: u8,
}

/// `GET /subscription-plans`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。
pub async fn list_subscription_plans(
    State(state): State<Arc<SubscriptionPlansState>>,
) -> Json<SubscriptionPlanListBody> {
    Json(SubscriptionPlanListBody {
        plans: state
            .list_subscription_plans
            .execute()
            .into_iter()
            .map(|plan| SubscriptionPlanDetailBody {
                plan: subscription_plan_to_contract(plan),
                live_lessons_per_month: plan.live_lessons_per_month(),
            })
            .collect(),
    })
}

/// 契約プランの一覧（student-app の `GET /subscription-plans`）のルーター。組み立て用のクレートは他の機能のルーターと
/// `merge` する（同じ検証器を渡す）。
///
/// `GET /subscription-plans` は生徒だけ。
pub fn subscription_plans_router<T>(
    list_subscription_plans: ListSubscriptionPlans,
    tokens: T,
) -> Router
where
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/subscription-plans", get(self::list_subscription_plans))
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(SubscriptionPlansState {
            list_subscription_plans,
        }))
        .layer(TokenVerification::layer(tokens))
}
