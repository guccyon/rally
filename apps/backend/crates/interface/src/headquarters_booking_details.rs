//! `GET /bookings/{booking_id}`。admin-web の `getBooking`（本部が見る予約 1 件の詳細）。本部だけが呼べる。同じパスの
//! 下の student-app の `POST /bookings/{booking_id}/confirm`・`/cancel`（生徒だけ）は [`crate::bookings`]。

use std::sync::Arc;

use application::{
    identity::TokenVerifier,
    scheduling::{
        BookingCoachDisplayNameLookup, BookingRepository, GetBooking, StudentDisplayNameLookup,
        booking_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use domain::{identity::Role, scheduling::BookingStatusChange};
use serde::Serialize;

use crate::{
    auth::{TokenVerification, require_role},
    bookings::{BookingBody, booking_body},
    contract::{booking_status_to_contract, date_time_to_contract},
    error::ApiError,
    scheduling_error::booking_not_found,
    slots::{AdminWebSlotBody, admin_web_slot_body},
};

/// `GET /bookings/{booking_id}` のハンドラーが使うユースケース。
pub struct HeadquartersBookingDetailState<B, K, D> {
    pub get_booking: GetBooking<B, K, D>,
}

/// admin-web の `HeadquartersBookingDetail`。コーチと生徒の表示名は任意の項目で、引けなければ載せない。
#[derive(Serialize)]
pub struct HeadquartersBookingDetailBody {
    booking: BookingBody,
    slot: AdminWebSlotBody,
    status_changes: Vec<BookingStatusChangeBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
}

/// admin-web の `BookingStatusChange`。
#[derive(Serialize)]
pub struct BookingStatusChangeBody {
    status: &'static str,
    changed_at: String,
}

/// `GET /bookings/{booking_id}`。本部だけが呼べる（役割は `require_role` の層が確かめる）。`booking_id` が UUID でない・
/// 予約がないときは 404（失敗の応答は [`crate::scheduling_error`] が決める）。
pub async fn get_booking<B, K, D>(
    State(state): State<Arc<HeadquartersBookingDetailState<B, K, D>>>,
    Path(booking_id): Path<String>,
) -> Result<Json<HeadquartersBookingDetailBody>, ApiError>
where
    B: BookingRepository,
    K: BookingCoachDisplayNameLookup,
    D: StudentDisplayNameLookup,
{
    let booking_id = booking_id_uuid::parse(&booking_id).map_err(|_| booking_not_found())?;
    let detail = state
        .get_booking
        .execute(booking_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(HeadquartersBookingDetailBody {
        booking: booking_body(&detail.booking)?,
        slot: admin_web_slot_body(&detail.slot, None)?,
        status_changes: detail
            .booking
            .status_changes()
            .iter()
            .map(status_change_body)
            .collect::<Result<_, ApiError>>()?,
        coach_display_name: detail
            .coach_display_name
            .map(|name| name.as_str().to_string()),
        student_display_name: detail
            .student_display_name
            .map(|name| name.as_str().to_string()),
    }))
}

fn status_change_body(change: &BookingStatusChange) -> Result<BookingStatusChangeBody, ApiError> {
    Ok(BookingStatusChangeBody {
        status: booking_status_to_contract(change.status()),
        changed_at: date_time_to_contract(change.changed_at())?,
    })
}

/// 本部が見る予約 1 件の詳細（admin-web の `GET /bookings/{booking_id}`）のルーター。組み立て用のクレートは他の機能の
/// ルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /bookings/{booking_id}` は本部だけ（コーチ・生徒は 403）。
pub fn headquarters_booking_detail_router<B, K, D, T>(
    get_booking: GetBooking<B, K, D>,
    tokens: T,
) -> Router
where
    B: BookingRepository + 'static,
    K: BookingCoachDisplayNameLookup + 'static,
    D: StudentDisplayNameLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/bookings/{booking_id}", get(self::get_booking::<B, K, D>))
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(HeadquartersBookingDetailState { get_booking }))
        .layer(TokenVerification::layer(tokens))
}
