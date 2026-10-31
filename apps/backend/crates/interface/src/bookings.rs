//! `/bookings`。student-app の `POST /bookings`（`createBooking`）と
//! `POST /bookings/{booking_id}/confirm`（`confirmBooking`）・`POST /bookings/{booking_id}/cancel`
//! （`cancelBooking`）。どれも生徒だけが呼べる。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    catalog::lesson_menu_id_uuid,
    clock::Clock,
    domain_event::DomainEventPublisher,
    identity::account_id_uuid,
    scheduling::{
        BookingRepository, CancelBooking, CancelBookingCommand, CancelBookingError, ConfirmBooking,
        ConfirmBookingCommand, ConfirmBookingError, CreateBooking, CreateBookingCommand,
        CreateBookingError, LessonMenuLookup, QuotaGate, booking_id_uuid, slot_id_uuid,
    },
};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
};
use domain::{
    DomainError,
    scheduling::{Booking, BookingLessonMenu, SlotStatus},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedAccount,
    contract::{booking_status_to_contract, date_time_to_contract},
    error::ApiError,
    message,
};

/// `/bookings` のハンドラーが使うユースケース。
pub struct BookingsState<B, J, G, A, P, C> {
    pub create_booking: CreateBooking<B, J, A, C>,
    pub confirm_booking: ConfirmBooking<B, G, A, P, C>,
    pub cancel_booking: CancelBooking<B, A, P, C>,
}

/// ハンドラーが共有する [`BookingsState`]。
type Shared<B, J, G, A, P, C> = Arc<BookingsState<B, J, G, A, P, C>>;

/// student-app の `CreateBookingRequest`。
#[derive(Deserialize)]
pub struct CreateBookingRequest {
    slot_id: String,
    is_trial: bool,
}

/// student-app・admin-web の `Booking`。
#[derive(Serialize)]
pub struct BookingBody {
    booking_id: String,
    slot_id: String,
    student_id: String,
    coach_id: String,
    is_trial: bool,
    status: &'static str,
    starts_at: String,
    created_at: String,
    confirmed_at: Option<String>,
    cancelled_at: Option<String>,
    /// 予約した時点のレッスンメニュー。メニューなしの予約は null
    lesson_menu: Option<BookingLessonMenuBody>,
}

/// student-app・admin-web の `BookingLessonMenu`（予約した時点のレッスンメニューの ID・名前・1 回の時間）。
#[derive(Serialize)]
pub struct BookingLessonMenuBody {
    lesson_menu_id: String,
    name: String,
    duration_minutes: Option<u8>,
}

/// `POST /bookings`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。予約する生徒はトークンのアカウント。
pub async fn create_booking<B, J, G, A, P, C>(
    State(state): State<Shared<B, J, G, A, P, C>>,
    account: AuthenticatedAccount,
    request: Result<Json<CreateBookingRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<BookingBody>), ApiError>
where
    B: BookingRepository,
    J: LessonMenuLookup,
    G: QuotaGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let command = CreateBookingCommand {
        student_id: account.account_id(),
        slot_id: slot_id_uuid::parse(&request.slot_id)
            .map_err(|_| ApiError::validation_failed("slot_id が UUID でない"))?,
        is_trial: request.is_trial,
    };

    let booking = state
        .create_booking
        .execute(command)
        .await
        .map_err(|error| match error {
            CreateBookingError::SlotNotFound => slot_not_found(),
            // 閉じた枠は生徒の検索に出ないので、ない枠と同じに扱う
            CreateBookingError::Invalid(DomainError::SlotNotOpen {
                status: SlotStatus::Closed,
            }) => slot_not_found(),
            // 満席と、同じ生徒の同じ枠への 2 回目は、どちらも 409 slot_already_booked（文言で分ける）
            CreateBookingError::Invalid(
                error @ (DomainError::SlotAlreadyBooked
                | DomainError::SlotAlreadyBookedByStudent
                | DomainError::SlotNotOpen { .. }),
            ) => ApiError::conflict("slot_already_booked", message::domain_error(&error)),
            CreateBookingError::Invalid(error @ DomainError::TrialAlreadyUsed) => {
                ApiError::conflict("trial_already_used", message::domain_error(&error))
            }
            CreateBookingError::Invalid(error) => ApiError::from(error),
            CreateBookingError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok((StatusCode::CREATED, Json(booking_body(&booking)?)))
}

/// `POST /bookings/{booking_id}/confirm`。生徒だけが呼べ、確定できるのは自分の予約だけ。
/// 月額の生徒のビデオ通話の暦月の枠が残っていなければ 409 `live_lesson_quota_exceeded`（不変条件 1）。
pub async fn confirm_booking<B, J, G, A, P, C>(
    State(state): State<Shared<B, J, G, A, P, C>>,
    account: AuthenticatedAccount,
    Path(booking_id): Path<String>,
) -> Result<Json<BookingBody>, ApiError>
where
    B: BookingRepository,
    J: LessonMenuLookup,
    G: QuotaGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    // 契約の confirmBooking に 422 はないので、UUID でない ID はない予約として扱う
    let booking_id = booking_id_uuid::parse(&booking_id).map_err(|_| booking_not_found())?;

    let booking = state
        .confirm_booking
        .execute(ConfirmBookingCommand {
            student_id: account.account_id(),
            booking_id,
        })
        .await
        .map_err(|error| match error {
            // 別の生徒の予約は、あることも知らせない
            ConfirmBookingError::BookingNotFound
            | ConfirmBookingError::Invalid(DomainError::BookingOfAnotherStudent) => {
                booking_not_found()
            }
            ConfirmBookingError::Invalid(
                error @ (DomainError::BookingNotPending { .. } | DomainError::SlotNotOpen { .. }),
            ) => ApiError::conflict("invalid_booking_state", message::domain_error(&error)),
            ConfirmBookingError::Invalid(error @ DomainError::LiveLessonQuotaExceeded) => {
                ApiError::conflict("live_lesson_quota_exceeded", message::domain_error(&error))
            }
            ConfirmBookingError::Invalid(error) => ApiError::internal(error),
            ConfirmBookingError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(booking_body(&booking)?))
}

/// `POST /bookings/{booking_id}/cancel`。生徒だけが呼べ、キャンセルできるのは自分の予約だけ。
///
/// 回数を消費するかはレスポンスに含めない（契約。判定は境界間イベント `BookingCancelled` の
/// `consumes_quota` で entitlement に渡す）。リクエストの本文（任意の `reason`）は読まない。
pub async fn cancel_booking<B, J, G, A, P, C>(
    State(state): State<Shared<B, J, G, A, P, C>>,
    account: AuthenticatedAccount,
    Path(booking_id): Path<String>,
) -> Result<Json<BookingBody>, ApiError>
where
    B: BookingRepository,
    J: LessonMenuLookup,
    G: QuotaGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    // 契約の cancelBooking に 422 はないので、UUID でない ID はない予約として扱う
    let booking_id = booking_id_uuid::parse(&booking_id).map_err(|_| booking_not_found())?;

    let booking = state
        .cancel_booking
        .execute(CancelBookingCommand {
            student_id: account.account_id(),
            booking_id,
        })
        .await
        .map_err(|error| match error {
            // 別の生徒の予約は、あることも知らせない
            CancelBookingError::BookingNotFound
            | CancelBookingError::Invalid(DomainError::BookingOfAnotherStudent) => {
                booking_not_found()
            }
            CancelBookingError::Invalid(error @ DomainError::BookingAlreadyCancelled) => {
                ApiError::conflict("invalid_booking_state", message::domain_error(&error))
            }
            CancelBookingError::Invalid(error) => ApiError::internal(error),
            CancelBookingError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(booking_body(&booking)?))
}

fn slot_not_found() -> ApiError {
    ApiError::not_found("予約できるスロットがない")
}

fn booking_not_found() -> ApiError {
    ApiError::not_found("予約がない")
}

pub(crate) fn booking_body(booking: &Booking) -> Result<BookingBody, ApiError> {
    Ok(BookingBody {
        booking_id: booking_id_uuid::format(booking.id()),
        slot_id: slot_id_uuid::format(booking.slot_id()),
        student_id: account_id_uuid::format(booking.student_id()),
        coach_id: account_id_uuid::format(booking.coach_id()),
        is_trial: booking.is_trial(),
        status: booking_status_to_contract(booking.status()),
        starts_at: date_time_to_contract(booking.starts_at())?,
        created_at: date_time_to_contract(booking.created_at())?,
        confirmed_at: booking
            .confirmed_at()
            .map(date_time_to_contract)
            .transpose()?,
        cancelled_at: booking
            .cancelled_at()
            .map(date_time_to_contract)
            .transpose()?,
        lesson_menu: booking.lesson_menu().map(booking_lesson_menu_body),
    })
}

fn booking_lesson_menu_body(menu: &BookingLessonMenu) -> BookingLessonMenuBody {
    BookingLessonMenuBody {
        lesson_menu_id: lesson_menu_id_uuid::format(menu.id()),
        name: menu.name().as_str().to_string(),
        duration_minutes: menu.duration().map(|d| d.minutes()),
    }
}
