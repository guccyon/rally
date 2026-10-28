//! `/bookings/me`。student-app の `GET /bookings/me`（`listMyBookings`。自分の予約の一覧）。生徒だけが呼べる。

use std::sync::Arc;

use application::scheduling::{BookingRepository, CoachIntroductionLookup, ListMyBookings};
use axum::{Json, extract::State};
use serde::Serialize;

use crate::{
    auth::AuthenticatedAccount,
    bookings::{BookingBody, booking_body},
    error::ApiError,
    slots::{StudentAppSlotBody, student_app_slot_body},
};

/// `/bookings/me` のハンドラーが使うユースケース。
pub struct MyBookingsState<B, I> {
    pub list_my_bookings: ListMyBookings<B, I>,
}

/// student-app の `MyBookingList`。
#[derive(Serialize)]
pub struct MyBookingListBody {
    bookings: Vec<MyBookingBody>,
}

/// student-app の `MyBooking`。
#[derive(Serialize)]
pub struct MyBookingBody {
    booking: BookingBody,
    slot: StudentAppSlotBody,
}

/// `GET /bookings/me`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンのアカウントの予約。
pub async fn list_my_bookings<B, I>(
    State(state): State<Arc<MyBookingsState<B, I>>>,
    account: AuthenticatedAccount,
) -> Result<Json<MyBookingListBody>, ApiError>
where
    B: BookingRepository,
    I: CoachIntroductionLookup,
{
    let list = state
        .list_my_bookings
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(MyBookingListBody {
        bookings: list
            .items
            .iter()
            .map(|(booking, slot)| {
                Ok(MyBookingBody {
                    booking: booking_body(booking)?,
                    slot: student_app_slot_body(slot, list.coaches.get(&slot.coach_id()))?,
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}
