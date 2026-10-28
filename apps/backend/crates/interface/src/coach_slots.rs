//! `/coaches/me/slots`。admin-web の `GET /coaches/me/slots`（`listMyCoachSlots`。コーチの自分の枠の一覧）。
//! コーチだけが呼べる。

use std::sync::Arc;

use application::scheduling::{BookingRepository, ListCoachSlots, SlotRepository};
use axum::{Json, extract::State};
use serde::Serialize;

use crate::{
    auth::AuthenticatedAccount,
    error::ApiError,
    slots::{AdminWebSlotBody, admin_web_slot_body},
};

/// `/coaches/me/slots` のハンドラーが使うユースケース。
pub struct CoachSlotsState<S, B> {
    pub list_coach_slots: ListCoachSlots<S, B>,
}

/// admin-web の `CoachSlotList`。
#[derive(Serialize)]
pub struct CoachSlotListBody {
    slots: Vec<CoachSlotBody>,
}

/// admin-web の `CoachSlot`。
#[derive(Serialize)]
pub struct CoachSlotBody {
    slot: AdminWebSlotBody,
    has_active_booking: bool,
}

/// `GET /coaches/me/slots`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンの
/// アカウントの枠。
pub async fn list_my_coach_slots<S, B>(
    State(state): State<Arc<CoachSlotsState<S, B>>>,
    account: AuthenticatedAccount,
) -> Result<Json<CoachSlotListBody>, ApiError>
where
    S: SlotRepository,
    B: BookingRepository,
{
    let list = state
        .list_coach_slots
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(CoachSlotListBody {
        slots: list
            .items
            .iter()
            .map(|(slot, has_active_booking)| {
                Ok(CoachSlotBody {
                    slot: admin_web_slot_body(slot)?,
                    has_active_booking: *has_active_booking,
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}
