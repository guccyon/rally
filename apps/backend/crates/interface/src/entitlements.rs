//! `/entitlements`。student-app の `GET /entitlements/me`（`getMyEntitlement`）。生徒だけが呼べる。

use std::sync::Arc;

use application::{
    clock::Clock,
    entitlement::{
        EntitlementRepository, GetEntitlement, GetEntitlementError, GetEntitlementQuery,
        entitlement_id_uuid,
    },
    identity::{TokenVerifier, account_id_uuid},
};
use axum::{Json, Router, extract::State, routing::get};
use domain::{
    entitlement::{Entitlement, LiveLessonQuota},
    identity::Role,
};
use serde::Serialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::{
        contract_type_to_contract, sport_to_contract, subscription_plan_to_contract,
        year_month_to_contract,
    },
    entitlement_error,
    error::ApiError,
};

/// `/entitlements` のハンドラーが使うユースケース。
pub struct EntitlementsState<E, C> {
    pub get_entitlement: GetEntitlement<E, C>,
}

/// student-app の `Entitlement`。
#[derive(Serialize)]
pub struct EntitlementBody {
    entitlement_id: String,
    student_id: String,
    contract_type: &'static str,
    sport: &'static str,
    plan: Option<&'static str>,
    unlimited_video_and_chat: bool,
    live_lesson_quota: Option<LiveLessonQuotaBody>,
    remaining_video_reviews: Option<u32>,
}

/// student-app の `LiveLessonQuota`。`limit` は契約プランで決まる暦月の上限。
#[derive(Serialize)]
pub struct LiveLessonQuotaBody {
    month: String,
    limit: u8,
    used: u8,
    remaining: u8,
}

/// `GET /entitlements/me`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンのアカウント。
pub async fn get_my_entitlement<E, C>(
    State(state): State<Arc<EntitlementsState<E, C>>>,
    account: AuthenticatedAccount,
) -> Result<Json<EntitlementBody>, ApiError>
where
    E: EntitlementRepository,
    C: Clock,
{
    let entitlement = state
        .get_entitlement
        .execute(GetEntitlementQuery {
            student_id: account.account_id(),
        })
        .await
        .map_err(|error| match error {
            GetEntitlementError::NotFound => entitlement_error::entitlement_not_found(),
            GetEntitlementError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(entitlement_body(&entitlement)))
}

pub(crate) fn entitlement_body(entitlement: &Entitlement) -> EntitlementBody {
    EntitlementBody {
        entitlement_id: entitlement_id_uuid::format(entitlement.id()),
        student_id: account_id_uuid::format(entitlement.student_id()),
        contract_type: contract_type_to_contract(entitlement.contract_type()),
        sport: sport_to_contract(entitlement.sport()),
        plan: entitlement.plan().map(subscription_plan_to_contract),
        unlimited_video_and_chat: entitlement.unlimited_video_and_chat(),
        live_lesson_quota: entitlement.live_lesson_quota().map(live_lesson_quota_body),
        remaining_video_reviews: entitlement.remaining_video_reviews(),
    }
}

fn live_lesson_quota_body(quota: &LiveLessonQuota) -> LiveLessonQuotaBody {
    LiveLessonQuotaBody {
        month: year_month_to_contract(quota.month()),
        limit: quota.limit(),
        used: quota.used(),
        remaining: quota.remaining(),
    }
}

/// 受講権の照会（student-app の `GET /entitlements/me`）のルーター。組み立て用のクレートは他の機能のルーターと
/// `merge` する（同じ検証器を渡す）。
///
/// `GET /entitlements/me` は生徒だけ。
pub fn entitlements_router<E, C, T>(get_entitlement: GetEntitlement<E, C>, tokens: T) -> Router
where
    E: EntitlementRepository + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/entitlements/me", get(get_my_entitlement::<E, C>))
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(EntitlementsState { get_entitlement }))
        .layer(TokenVerification::layer(tokens))
}
