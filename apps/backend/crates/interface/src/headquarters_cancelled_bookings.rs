//! `GET /bookings/cancelled`。admin-web の `listCancelledBookings`（本部が見るキャンセルされた予約の一覧）。本部だけが
//! 呼べる。同じ階層の `GET /bookings/{booking_id}`（予約 1 件の詳細）は [`crate::headquarters_booking_details`] で、
//! 固定のパスのこちらが先に当たる。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    scheduling::{
        BookingCoachDisplayNameLookup, BookingRepository, CancelledBookingList,
        ListCancelledBookings, ListCancelledBookingsQuery, StudentDisplayNameLookup,
        booking_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::{identity::Role, scheduling::Booking};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::{
        booking_canceller_to_contract, date_from_contract, date_time_to_contract, date_to_contract,
    },
    error::ApiError,
};

/// `GET /bookings/cancelled` のハンドラーが使うユースケース。
pub struct HeadquartersCancelledBookingsState<B, K, D, C> {
    pub list_cancelled_bookings: ListCancelledBookings<B, K, D, C>,
}

/// admin-web の `listCancelledBookings` のクエリ。
#[derive(Deserialize)]
pub struct ListCancelledBookingsParams {
    from: Option<String>,
    to: Option<String>,
}

/// admin-web の `CancelledBookingList`。
#[derive(Serialize)]
pub struct CancelledBookingListBody {
    from: String,
    to: String,
    bookings: Vec<CancelledBookingBody>,
}

/// admin-web の `CancelledBooking`。コーチと生徒の表示名は任意の項目で、引けなければ載せない。`cancelled_by` は
/// 記録がなければ null で、項目は必ず載せる。
#[derive(Serialize)]
pub struct CancelledBookingBody {
    booking_id: String,
    student_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
    coach_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    starts_at: String,
    cancelled_at: String,
    cancelled_by: Option<&'static str>,
}

/// `GET /bookings/cancelled`。本部だけが呼べる（役割は `require_role` の層が確かめる）。`from`・`to` が `YYYY-MM-DD` の
/// 日付でないときと、期間の違反は 422（期間の違反の応答は [`crate::scheduling_error`] が決める。`listBookings` と同じ）。
pub async fn list_cancelled_bookings<B, K, D, C>(
    State(state): State<Arc<HeadquartersCancelledBookingsState<B, K, D, C>>>,
    _account: AuthenticatedAccount,
    params: Result<Query<ListCancelledBookingsParams>, QueryRejection>,
) -> Result<Json<CancelledBookingListBody>, ApiError>
where
    B: BookingRepository,
    K: BookingCoachDisplayNameLookup,
    D: StudentDisplayNameLookup,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let query = ListCancelledBookingsQuery {
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
        .list_cancelled_bookings
        .execute(query)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(CancelledBookingListBody {
        from: date_to_contract(list.from),
        to: date_to_contract(list.to),
        bookings: list
            .bookings
            .iter()
            .map(|booking| cancelled_booking_body(booking, &list))
            .collect::<Result<_, ApiError>>()?,
    }))
}

fn cancelled_booking_body(
    booking: &Booking,
    list: &CancelledBookingList,
) -> Result<CancelledBookingBody, ApiError> {
    let cancelled_at = booking
        .cancelled_at()
        .ok_or_else(|| ApiError::internal("キャンセルした時刻のない予約が一覧にある"))?;
    Ok(CancelledBookingBody {
        booking_id: booking_id_uuid::format(booking.id()),
        student_id: account_id_uuid::format(booking.student_id()),
        student_display_name: list
            .students
            .get(&booking.student_id())
            .map(|name| name.as_str().to_string()),
        coach_id: account_id_uuid::format(booking.coach_id()),
        coach_display_name: list
            .coaches
            .get(&booking.coach_id())
            .map(|name| name.as_str().to_string()),
        starts_at: date_time_to_contract(booking.starts_at())?,
        cancelled_at: date_time_to_contract(cancelled_at)?,
        cancelled_by: booking.cancelled_by().map(booking_canceller_to_contract),
    })
}

/// 本部が見るキャンセルされた予約の一覧（admin-web の `GET /bookings/cancelled`）のルーター。組み立て用のクレートは他の
/// 機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /bookings/cancelled` は本部だけ（コーチ・生徒は 403）。
pub fn headquarters_cancelled_bookings_router<B, K, D, C, T>(
    list_cancelled_bookings: ListCancelledBookings<B, K, D, C>,
    tokens: T,
) -> Router
where
    B: BookingRepository + 'static,
    K: BookingCoachDisplayNameLookup + 'static,
    D: StudentDisplayNameLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/bookings/cancelled",
            get(self::list_cancelled_bookings::<B, K, D, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(HeadquartersCancelledBookingsState {
            list_cancelled_bookings,
        }))
        .layer(TokenVerification::layer(tokens))
}
