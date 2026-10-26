//! `/bookings`。student-app の `POST /bookings`（`createBooking`）と
//! `POST /bookings/{booking_id}/confirm`（`confirmBooking`）。どちらも生徒だけが呼べる。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    domain_event::DomainEventPublisher,
    identity::account_id_uuid,
    scheduling::{
        BookingRepository, ConfirmBooking, ConfirmBookingCommand, ConfirmBookingError,
        CreateBooking, CreateBookingCommand, CreateBookingError, booking_id_uuid, slot_id_uuid,
    },
};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
};
use domain::{
    DomainError,
    scheduling::{Booking, SlotStatus},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedAccount,
    contract::{booking_status_to_contract, date_time_to_contract},
    error::ApiError,
    message,
};

/// `/bookings` のハンドラーが使うユースケース。
pub struct BookingsState<B, A, P, C> {
    pub create_booking: CreateBooking<B, A, C>,
    pub confirm_booking: ConfirmBooking<B, A, P, C>,
}

/// student-app の `CreateBookingRequest`。
#[derive(Deserialize)]
pub struct CreateBookingRequest {
    slot_id: String,
    is_trial: bool,
}

/// student-app の `Booking`。キャンセル（T09）はまだないので `cancelled_at` は常に null。
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
}

/// `POST /bookings`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。予約する生徒はトークンのアカウント。
pub async fn create_booking<B, A, P, C>(
    State(state): State<Arc<BookingsState<B, A, P, C>>>,
    account: AuthenticatedAccount,
    request: Result<Json<CreateBookingRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<BookingBody>), ApiError>
where
    B: BookingRepository,
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
            CreateBookingError::Invalid(
                error @ (DomainError::SlotAlreadyBooked | DomainError::SlotNotOpen { .. }),
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
pub async fn confirm_booking<B, A, P, C>(
    State(state): State<Arc<BookingsState<B, A, P, C>>>,
    account: AuthenticatedAccount,
    Path(booking_id): Path<String>,
) -> Result<Json<BookingBody>, ApiError>
where
    B: BookingRepository,
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
            ConfirmBookingError::Invalid(error) => ApiError::internal(error),
            ConfirmBookingError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(booking_body(&booking)?))
}

fn slot_not_found() -> ApiError {
    ApiError::not_found("予約できるスロットがない")
}

fn booking_not_found() -> ApiError {
    ApiError::not_found("予約がない")
}

fn booking_body(booking: &Booking) -> Result<BookingBody, ApiError> {
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
        cancelled_at: None,
    })
}
