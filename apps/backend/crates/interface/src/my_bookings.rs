//! `/bookings/me`。student-app の `GET /bookings/me`（`listMyBookings`。自分の予約の一覧）。生徒だけが呼べる。

use std::sync::Arc;

use application::{
    clock::Clock,
    scheduling::{BookingRepository, CoachIntroductionLookup, ListMyBookings, QuotaGate},
};
use axum::{Json, extract::State};
use serde::Serialize;

use crate::{
    auth::AuthenticatedAccount,
    bookings::{BookingBody, booking_body},
    contract::date_time_to_contract,
    error::ApiError,
    slots::{StudentAppSlotBody, student_app_slot_body},
};

/// `/bookings/me` のハンドラーが使うユースケース。
pub struct MyBookingsState<B, I, G, C> {
    pub list_my_bookings: ListMyBookings<B, I, G, C>,
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
    /// 今キャンセルしたらビデオ通話の暦月の枠を使うか。キャンセルできない予約（キャンセル済み）は null
    cancel_consumes_quota: Option<bool>,
    /// この時刻より前のキャンセルなら枠を使わない境目（開始の 24 時間前）。境目のない予約は null
    free_cancel_until: Option<String>,
}

/// `GET /bookings/me`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンのアカウントの予約。
pub async fn list_my_bookings<B, I, G, C>(
    State(state): State<Arc<MyBookingsState<B, I, G, C>>>,
    account: AuthenticatedAccount,
) -> Result<Json<MyBookingListBody>, ApiError>
where
    B: BookingRepository,
    I: CoachIntroductionLookup,
    G: QuotaGate,
    C: Clock,
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
                let cancellation = list.cancellations.get(&booking.id());
                Ok(MyBookingBody {
                    booking: booking_body(booking)?,
                    slot: student_app_slot_body(slot, list.coaches.get(&slot.coach_id()), None)?,
                    cancel_consumes_quota: cancellation.map(|preview| preview.consumes_quota),
                    free_cancel_until: cancellation
                        .and_then(|preview| preview.free_cancel_until)
                        .map(date_time_to_contract)
                        .transpose()?,
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}
