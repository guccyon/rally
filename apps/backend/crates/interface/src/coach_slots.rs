//! `/coaches/me/slots`。admin-web の `GET /coaches/me/slots`（`listMyCoachSlots`。コーチの自分の枠の一覧）。
//! コーチだけが呼べる。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::TokenVerifier,
    scheduling::{
        BookingRepository, LessonMenuLookup, ListCoachSlots, ListCoachSlotsQuery, SlotRepository,
    },
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::identity::Role;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::date_from_contract,
    error::ApiError,
    slots::{AdminWebSlotBody, admin_web_slot_body},
};

/// `/coaches/me/slots` のハンドラーが使うユースケース。
pub struct CoachSlotsState<S, J, B, C> {
    pub list_coach_slots: ListCoachSlots<S, J, B, C>,
}

/// admin-web の `listMyCoachSlots` のクエリ。
#[derive(Deserialize)]
pub struct ListMyCoachSlotsParams {
    from: Option<String>,
    to: Option<String>,
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
/// アカウントの枠。`from`・`to` が `YYYY-MM-DD` の日付でないときと、期間の違反は `listBookings` と同じく 422（失敗の応答は
/// [`crate::scheduling_error`] が決める）。
pub async fn list_my_coach_slots<S, J, B, C>(
    State(state): State<Arc<CoachSlotsState<S, J, B, C>>>,
    account: AuthenticatedAccount,
    params: Result<Query<ListMyCoachSlotsParams>, QueryRejection>,
) -> Result<Json<CoachSlotListBody>, ApiError>
where
    S: SlotRepository,
    J: LessonMenuLookup,
    B: BookingRepository,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let query = ListCoachSlotsQuery {
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
    };
    let list = state
        .list_coach_slots
        .execute(account.account_id(), query)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(CoachSlotListBody {
        slots: list
            .items
            .iter()
            .map(|(slot, has_active_booking)| {
                Ok(CoachSlotBody {
                    slot: admin_web_slot_body(slot, Some(&list.lesson_menus))?,
                    has_active_booking: *has_active_booking,
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}

/// コーチの自分の枠の一覧（admin-web の `GET /coaches/me/slots`）のルーター。組み立て用のクレートは他の機能の
/// ルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /coaches/me/slots` はコーチだけ。
pub fn coach_slots_router<S, J, B, C, T>(
    list_coach_slots: ListCoachSlots<S, J, B, C>,
    tokens: T,
) -> Router
where
    S: SlotRepository + 'static,
    J: LessonMenuLookup + 'static,
    B: BookingRepository + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/coaches/me/slots", get(list_my_coach_slots::<S, J, B, C>))
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(CoachSlotsState { list_coach_slots }))
        .layer(TokenVerification::layer(tokens))
}
