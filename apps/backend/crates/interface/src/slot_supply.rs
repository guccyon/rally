//! `/slot-supply`。admin-web の `GET /slot-supply`（`getSlotSupply`）。本部だけが呼べる。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::TokenVerifier,
    scheduling::{BookingRepository, GetSlotSupply},
};
use axum::{Json, Router, extract::State, routing::get};
use domain::{identity::Role, scheduling::SlotSupply};
use serde::Serialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::{date_time_to_contract, slot_supply_status_to_contract, sport_to_contract},
    error::ApiError,
};

/// `/slot-supply` のハンドラーが使うユースケース。
pub struct SlotSupplyState<B, C> {
    pub get_slot_supply: GetSlotSupply<B, C>,
}

/// admin-web の `SlotSupplyReport`。
#[derive(Serialize)]
pub struct SlotSupplyReportBody {
    window_start: String,
    window_end: String,
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

/// `GET /slot-supply`。本部だけが呼べる（役割は `require_role` の層が確かめる）。
pub async fn get_slot_supply<B, C>(
    State(state): State<Arc<SlotSupplyState<B, C>>>,
    _account: AuthenticatedAccount,
) -> Result<Json<SlotSupplyReportBody>, ApiError>
where
    B: BookingRepository,
    C: Clock,
{
    let report = state
        .get_slot_supply
        .execute()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(SlotSupplyReportBody {
        window_start: date_time_to_contract(report.window_start)?,
        window_end: date_time_to_contract(report.window_end)?,
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
pub fn slot_supply_router<B, C, T>(get_slot_supply: GetSlotSupply<B, C>, tokens: T) -> Router
where
    B: BookingRepository + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/slot-supply", get(self::get_slot_supply::<B, C>))
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(SlotSupplyState { get_slot_supply }))
        .layer(TokenVerification::layer(tokens))
}
