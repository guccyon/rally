//! `GET /bookings`。admin-web の `listBookings`（本部が見る予約の一覧）。本部だけが呼べる。同じパスの student-app の
//! `POST /bookings`（予約の作成。生徒だけ）は [`crate::bookings`]。

use std::sync::Arc;

use application::{
    clock::Clock,
    scheduling::{
        BookingCoachDisplayNameLookup, BookingRepository, ListBookings, ListBookingsQuery,
        StudentDisplayNameLookup,
    },
};
use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedAccount,
    bookings::{BookingBody, booking_body},
    contract::{date_from_contract, date_to_contract},
    error::ApiError,
};

/// `GET /bookings`（本部）のハンドラーが使うユースケース。
pub struct HeadquartersBookingsState<B, K, D, C> {
    pub list_bookings: ListBookings<B, K, D, C>,
}

/// admin-web の `listBookings` のクエリ。
#[derive(Deserialize)]
pub struct ListBookingsParams {
    from: Option<String>,
    to: Option<String>,
}

/// admin-web の `HeadquartersBookingList`。
#[derive(Serialize)]
pub struct HeadquartersBookingListBody {
    from: String,
    to: String,
    bookings: Vec<HeadquartersBookingBody>,
}

/// admin-web の `HeadquartersBooking`。コーチと生徒の表示名は任意の項目で、引けなければ載せない。
#[derive(Serialize)]
pub struct HeadquartersBookingBody {
    booking: BookingBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
}

/// `GET /bookings`。本部だけが呼べる（役割は `require_role` の層が確かめる）。`from`・`to` が `YYYY-MM-DD` の日付で
/// ないときと、期間の違反は 422（失敗の応答は [`crate::scheduling_error`] が決める）。
pub async fn list_bookings<B, K, D, C>(
    State(state): State<Arc<HeadquartersBookingsState<B, K, D, C>>>,
    _account: AuthenticatedAccount,
    params: Result<Query<ListBookingsParams>, QueryRejection>,
) -> Result<Json<HeadquartersBookingListBody>, ApiError>
where
    B: BookingRepository,
    K: BookingCoachDisplayNameLookup,
    D: StudentDisplayNameLookup,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let query = ListBookingsQuery {
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
        .list_bookings
        .execute(query)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(HeadquartersBookingListBody {
        from: date_to_contract(list.from),
        to: date_to_contract(list.to),
        bookings: list
            .bookings
            .iter()
            .map(|booking| {
                Ok(HeadquartersBookingBody {
                    booking: booking_body(booking)?,
                    coach_display_name: list
                        .coaches
                        .get(&booking.coach_id())
                        .map(|name| name.as_str().to_string()),
                    student_display_name: list
                        .students
                        .get(&booking.student_id())
                        .map(|name| name.as_str().to_string()),
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}
