//! `/slot-supply`。admin-web の `GET /slot-supply`（`getSlotSupply`）。本部だけが呼べる。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::TokenVerifier,
    scheduling::{BookingRepository, GetSlotSupply, GetSlotSupplyQuery, SlotCoachAccountLookup},
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::{identity::Role, scheduling::SlotSupply};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::{
        account_id_from_contract, admin_web_sport, date_from_contract, date_time_to_contract,
        date_to_contract, slot_supply_status_to_contract, sport_to_contract,
    },
    error::ApiError,
};

/// `/slot-supply` のハンドラーが使うユースケース。
pub struct SlotSupplyState<B, A, C> {
    pub get_slot_supply: GetSlotSupply<B, A, C>,
}

/// admin-web の `getSlotSupply` のクエリ。
#[derive(Deserialize)]
pub struct GetSlotSupplyParams {
    from: Option<String>,
    to: Option<String>,
    coach_id: Option<String>,
    sport: Option<String>,
}

/// admin-web の `SlotSupplyReport`。`from`・`to` は日付で期間を指定したときだけ載せる。
#[derive(Serialize)]
pub struct SlotSupplyReportBody {
    window_start: String,
    window_end: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    sports: Vec<SlotSupplyBody>,
}

/// admin-web の `SlotSupply`。
#[derive(Serialize)]
pub struct SlotSupplyBody {
    sport: &'static str,
    status: &'static str,
    published_slots: u32,
    booking_students: u32,
}

/// `GET /slot-supply`。本部だけが呼べる（役割は `require_role` の層が確かめる）。`from`・`to` が `YYYY-MM-DD` の
/// 日付でないとき・`coach_id` が UUID でないときと、期間の違反は 422（期間の違反の応答は [`crate::scheduling_error`] が
/// 決める。`listBookings` と同じ）。ない・コーチでない `coach_id` は 404 にせず、どのスポーツも 0 の需給。`sport` が
/// `Sport` の値でなければ 422（`listPendingVideoReviews` の `sport` と同じ）。提供するコーチのいない競技も、どのスポーツも
/// 0 の需給。
pub async fn get_slot_supply<B, A, C>(
    State(state): State<Arc<SlotSupplyState<B, A, C>>>,
    _account: AuthenticatedAccount,
    params: Result<Query<GetSlotSupplyParams>, QueryRejection>,
) -> Result<Json<SlotSupplyReportBody>, ApiError>
where
    B: BookingRepository,
    A: SlotCoachAccountLookup,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let query = GetSlotSupplyQuery {
        from: params
            .from
            .as_deref()
            .map(|value| date_from_contract("from", value))
            .transpose()?,
        to: params
            .to
            .as_deref()
            .map(|value| date_from_contract("to", value))
            .transpose()?,
        coach_id: params
            .coach_id
            .as_deref()
            .map(|id| account_id_from_contract("coach_id", id))
            .transpose()?,
        sport: params.sport.as_deref().map(admin_web_sport).transpose()?,
    };
    let report = state
        .get_slot_supply
        .execute(query)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(SlotSupplyReportBody {
        window_start: date_time_to_contract(report.window_start)?,
        window_end: date_time_to_contract(report.window_end)?,
        from: report.dates.map(|(from, _)| date_to_contract(from)),
        to: report.dates.map(|(_, to)| date_to_contract(to)),
        sports: report.sports.iter().map(slot_supply_body).collect(),
    }))
}

fn slot_supply_body(supply: &SlotSupply) -> SlotSupplyBody {
    SlotSupplyBody {
        sport: sport_to_contract(supply.sport()),
        status: slot_supply_status_to_contract(supply.status()),
        published_slots: supply.published_slots(),
        booking_students: supply.booking_students(),
    }
}

/// スポーツごとの予約枠の需給（admin-web の `GET /slot-supply`）のルーター。組み立て用のクレートは他の機能の
/// ルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /slot-supply` は本部だけ。
pub fn slot_supply_router<B, A, C, T>(get_slot_supply: GetSlotSupply<B, A, C>, tokens: T) -> Router
where
    B: BookingRepository + 'static,
    A: SlotCoachAccountLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/slot-supply", get(self::get_slot_supply::<B, A, C>))
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(SlotSupplyState { get_slot_supply }))
        .layer(TokenVerification::layer(tokens))
}
