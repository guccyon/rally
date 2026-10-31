//! `/subscription-plans`。student-app の `GET /subscription-plans`（`listSubscriptionPlans`）。生徒だけが呼べる。

use std::sync::Arc;

use application::entitlement::ListSubscriptionPlans;
use axum::{Json, extract::State};
use serde::Serialize;

use crate::contract::subscription_plan_to_contract;

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
