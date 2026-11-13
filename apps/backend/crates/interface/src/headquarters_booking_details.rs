//! `GET /bookings/{booking_id}`。admin-web の `getBooking`（本部が見る予約 1 件の詳細）。本部だけが呼べる。同じパスの
//! 下の student-app の `POST /bookings/{booking_id}/confirm`・`/cancel`（生徒だけ）は [`crate::bookings`]。

use std::{sync::Arc, time::SystemTime};

use application::{
    identity::TokenVerifier,
    scheduling::{
        BookingCoachDisplayNameLookup, BookingRepository, CoachLeaveRepository, GetBooking,
        StudentDisplayNameLookup, booking_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use domain::{
    identity::Role,
    scheduling::{BookingCancelReason, BookingStatus, BookingStatusChange, CoachLeave},
};
use serde::Serialize;

use crate::{
    auth::{TokenVerification, require_role},
    bookings::{BookingBody, booking_body},
    coach_leaves::{CoachLeaveBody, coach_leave_body},
    contract::{
        booking_cancel_reason_to_contract, booking_canceller_to_contract,
        booking_status_to_contract, date_time_to_contract,
    },
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

/// admin-web の `BookingStatusChange`。キャンセルの記録だけがキャンセルの経緯（[`CancellationBody`]）を載せ、確定の
/// 記録は欄ごと省く。
#[derive(Serialize)]
pub struct BookingStatusChangeBody {
    status: &'static str,
    changed_at: String,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    cancellation: Option<CancellationBody>,
}

/// admin-web の `BookingStatusChange` のキャンセルの経緯の欄。記録のない欄は省かず null。
#[derive(Serialize)]
pub struct CancellationBody {
    cancelled_by: Option<&'static str>,
    consumes_quota: Option<bool>,
    hours_before_start: i64,
    cancel_reason: Option<&'static str>,
    coach_leave: Option<CoachLeaveBody>,
}

/// `GET /bookings/{booking_id}`。本部だけが呼べる（役割は `require_role` の層が確かめる）。`booking_id` が UUID でない・
/// 予約がないときは 404（失敗の応答は [`crate::scheduling_error`] が決める）。
pub async fn get_booking<B, K, D>(
    State(state): State<Arc<HeadquartersBookingDetailState<B, K, D>>>,
    Path(booking_id): Path<String>,
) -> Result<Json<HeadquartersBookingDetailBody>, ApiError>
where
    B: BookingRepository + CoachLeaveRepository,
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
            .map(|change| {
                status_change_body(
                    change,
                    detail.booking.starts_at(),
                    detail.coach_leave.as_ref(),
                )
            })
            .collect::<Result<_, ApiError>>()?,
        coach_display_name: detail
            .coach_display_name
            .map(|name| name.as_str().to_string()),
        student_display_name: detail
            .student_display_name
            .map(|name| name.as_str().to_string()),
    }))
}

/// 状態が変わった記録 1 件。キャンセルの記録には、枠の開始 `starts_at` から数えた何時間前かと、理由がコーチのお休みなら
/// そのお休みの記録 `coach_leave` を載せる。
fn status_change_body(
    change: &BookingStatusChange,
    starts_at: SystemTime,
    coach_leave: Option<&CoachLeave>,
) -> Result<BookingStatusChangeBody, ApiError> {
    let cancellation = match change.status() {
        BookingStatus::Cancelled => Some(CancellationBody {
            cancelled_by: change.cancelled_by().map(booking_canceller_to_contract),
            consumes_quota: change
                .cancellation_decision()
                .map(|decision| decision.consumes_quota),
            hours_before_start: hours_before_start(starts_at, change.changed_at()),
            cancel_reason: change
                .cancel_reason()
                .map(booking_cancel_reason_to_contract),
            coach_leave: match change.cancel_reason() {
                Some(BookingCancelReason::CoachLeave) => {
                    coach_leave.map(coach_leave_body).transpose()?
                }
                None => None,
            },
        }),
        BookingStatus::Pending | BookingStatus::Confirmed => None,
    };
    Ok(BookingStatusChangeBody {
        status: booking_status_to_contract(change.status()),
        changed_at: date_time_to_contract(change.changed_at())?,
        cancellation,
    })
}

/// `BookingStatusChange.hours_before_start`。`at` から枠の開始 `starts_at` までの時間を時間で数え、1 時間に満たない端数は
/// 0 に向けて切り捨てる（`at` が開始より後なら負。行動イベント `booking.cancelled` の同名の属性は負の側へ切り捨てるので
/// 別に数える）。
fn hours_before_start(starts_at: SystemTime, at: SystemTime) -> i64 {
    const SECONDS_PER_HOUR: u64 = 3600;
    match starts_at.duration_since(at) {
        Ok(before) => i64::try_from(before.as_secs() / SECONDS_PER_HOUR).unwrap_or(i64::MAX),
        Err(after) => {
            -i64::try_from(after.duration().as_secs() / SECONDS_PER_HOUR).unwrap_or(i64::MAX)
        }
    }
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
    B: BookingRepository + CoachLeaveRepository + 'static,
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn hours_before_start_truncates_toward_zero() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(100 * 3600);
        let minute = Duration::from_secs(60);
        let hour = 60 * minute;
        assert_eq!(hours_before_start(start, start - 23 * hour), 23);
        assert_eq!(
            hours_before_start(start, start - 23 * hour - 59 * minute),
            23
        );
        assert_eq!(hours_before_start(start, start - minute), 0);
        assert_eq!(hours_before_start(start, start), 0);
        assert_eq!(hours_before_start(start, start + minute), 0);
        assert_eq!(hours_before_start(start, start + hour + minute), -1);
    }
}
