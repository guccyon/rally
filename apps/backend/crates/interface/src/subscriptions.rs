//! `/subscriptions`。student-app の `POST /subscriptions`（`startSubscription`）。生徒だけが呼べる。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    domain_event::DomainEventPublisher,
    entitlement::{
        EntitlementRepository, StartSubscription, StartSubscriptionCommand, StartSubscriptionError,
        subscription_id_uuid,
    },
    identity::account_id_uuid,
};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
};
use domain::entitlement::Subscription;
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedAccount,
    contract::{
        date_time_to_contract, sport_to_contract, student_app_sport,
        subscription_plan_from_contract, subscription_plan_to_contract,
        subscription_status_to_contract,
    },
    entitlement_error,
    entitlements::{EntitlementBody, entitlement_body},
    error::ApiError,
};

/// `/subscriptions` のハンドラーが使うユースケース。
pub struct SubscriptionsState<E, A, P, C> {
    pub start_subscription: StartSubscription<E, A, P, C>,
}

/// student-app の `StartSubscriptionRequest`。
#[derive(Deserialize)]
pub struct StartSubscriptionRequest {
    sport: String,
    /// 省いたら standard
    #[serde(default)]
    plan: Option<String>,
    /// 決済（billing）は Gen1 の範囲外なので、受け取るだけで使わない
    #[allow(dead_code)]
    payment_method_token: String,
}

/// student-app の `Subscription`。
#[derive(Serialize)]
pub struct SubscriptionBody {
    subscription_id: String,
    student_id: String,
    sport: &'static str,
    plan: &'static str,
    status: &'static str,
    started_at: String,
}

/// student-app の `StartSubscriptionResponse`。
#[derive(Serialize)]
pub struct StartSubscriptionResponse {
    subscription: SubscriptionBody,
    entitlement: EntitlementBody,
}

/// `POST /subscriptions`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。契約するのはトークンのアカウント。
pub async fn start_subscription<E, A, P, C>(
    State(state): State<Arc<SubscriptionsState<E, A, P, C>>>,
    account: AuthenticatedAccount,
    request: Result<Json<StartSubscriptionRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<StartSubscriptionResponse>), ApiError>
where
    E: EntitlementRepository,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let command = StartSubscriptionCommand {
        student_id: account.account_id(),
        sport: student_app_sport(&request.sport)?,
        plan: subscription_plan_from_contract(request.plan.as_deref())?,
    };

    let (subscription, entitlement) =
        state
            .start_subscription
            .execute(command)
            .await
            .map_err(|error| match error {
                StartSubscriptionError::Invalid(error) => ApiError::from(error),
                StartSubscriptionError::AlreadySubscribed => {
                    entitlement_error::already_subscribed()
                }
                StartSubscriptionError::Unavailable(error) => ApiError::internal(error),
            })?;
    Ok((
        StatusCode::CREATED,
        Json(StartSubscriptionResponse {
            subscription: subscription_body(&subscription)?,
            entitlement: entitlement_body(&entitlement),
        }),
    ))
}

fn subscription_body(subscription: &Subscription) -> Result<SubscriptionBody, ApiError> {
    Ok(SubscriptionBody {
        subscription_id: subscription_id_uuid::format(subscription.id()),
        student_id: account_id_uuid::format(subscription.student_id()),
        sport: sport_to_contract(subscription.sport()),
        plan: subscription_plan_to_contract(subscription.plan()),
        status: subscription_status_to_contract(subscription.status()),
        started_at: date_time_to_contract(subscription.started_at())?,
    })
}
