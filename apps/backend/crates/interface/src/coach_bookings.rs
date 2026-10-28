//! `/coaches/me/bookings`。admin-web の `GET /coaches/me/bookings`（`listMyCoachBookings`。コーチの自分の予約の一覧）。
//! コーチだけが呼べる。

use std::sync::Arc;

use application::scheduling::{BookingRepository, ListCoachBookings, StudentDisplayNameLookup};
use axum::{Json, extract::State};
use serde::Serialize;

use crate::{
    auth::AuthenticatedAccount,
    bookings::{BookingBody, booking_body},
    error::ApiError,
    slots::{AdminWebSlotBody, admin_web_slot_body},
};

/// `/coaches/me/bookings` のハンドラーが使うユースケース。
pub struct CoachBookingsState<B, D> {
    pub list_coach_bookings: ListCoachBookings<B, D>,
}

/// admin-web の `CoachBookingList`。
#[derive(Serialize)]
pub struct CoachBookingListBody {
    bookings: Vec<CoachBookingBody>,
}

/// admin-web の `CoachBooking`。生徒の表示名は任意の項目で、引けなければ載せない。
#[derive(Serialize)]
pub struct CoachBookingBody {
    booking: BookingBody,
    slot: AdminWebSlotBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
}

/// `GET /coaches/me/bookings`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンの
/// アカウントのスロットへの予約。
pub async fn list_my_coach_bookings<B, D>(
    State(state): State<Arc<CoachBookingsState<B, D>>>,
    account: AuthenticatedAccount,
) -> Result<Json<CoachBookingListBody>, ApiError>
where
    B: BookingRepository,
    D: StudentDisplayNameLookup,
{
    let list = state
        .list_coach_bookings
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(CoachBookingListBody {
        bookings: list
            .items
            .iter()
            .map(|(booking, slot)| {
                Ok(CoachBookingBody {
                    booking: booking_body(booking)?,
                    slot: admin_web_slot_body(slot)?,
                    student_display_name: list
                        .students
                        .get(&booking.student_id())
                        .map(|name| name.as_str().to_string()),
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}
