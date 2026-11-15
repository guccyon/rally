//! `/bookings`。student-app の `POST /bookings`（`createBooking`）と
//! `POST /bookings/{booking_id}/confirm`（`confirmBooking`）・`POST /bookings/{booking_id}/cancel`
//! （`cancelBooking`）。どれも生徒だけが呼べる。

use std::sync::Arc;

use application::{
    analytics::AnalyticsSink,
    catalog::lesson_menu_id_uuid,
    clock::Clock,
    domain_event::DomainEventPublisher,
    identity::{TokenVerifier, account_id_uuid},
    scheduling::{
        BookingCoachDisplayNameLookup, BookingRepository, CancelBooking, CancelBookingCommand,
        ConfirmBooking, ConfirmBookingCommand, CreateBooking, CreateBookingCommand,
        DescribeBooking, DescribedBooking, LessonMenuLookup, QuotaGate, TrialUsageLookup,
        booking_id_uuid, slot_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    routing::post,
};
use domain::{
    identity::Role,
    profile::DisplayName,
    scheduling::{Booking, BookingLessonMenu, Slot},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::{booking_status_to_contract, date_time_to_contract, sport_to_contract},
    error::ApiError,
    scheduling_error::booking_not_found,
};

/// `/bookings` のハンドラーが使うユースケース。`describe_booking` は返す予約にスロットとコーチの表示名を添える。
pub struct BookingsState<B, J, M, G, A, P, C, K> {
    pub create_booking: CreateBooking<B, J, M, G, A, C>,
    pub confirm_booking: ConfirmBooking<B, G, A, P, C>,
    pub cancel_booking: CancelBooking<B, A, P, C>,
    pub describe_booking: DescribeBooking<B, K>,
}

/// ハンドラーが共有する [`BookingsState`] の取り出し。
type Shared<B, J, M, G, A, P, C, K> = State<Arc<BookingsState<B, J, M, G, A, P, C, K>>>;

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
    /// 予約したスロットの終了時刻。student-app の予約を返す操作だけが載せる（admin-web の `Booking` にはない）
    #[serde(skip_serializing_if = "Option::is_none")]
    ends_at: Option<String>,
    /// 予約したスロットのスポーツ。student-app の予約を返す操作だけが載せる
    #[serde(skip_serializing_if = "Option::is_none")]
    sport: Option<&'static str>,
    /// 予約のコーチの表示名。student-app の予約を返す操作だけが載せ、コーチのアカウントが引けなければ載せない
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
}

/// student-app・admin-web の `BookingLessonMenu`（予約した時点のレッスンメニューの ID・名前・1 回の時間）。
#[derive(Serialize)]
pub struct BookingLessonMenuBody {
    lesson_menu_id: String,
    name: String,
    duration_minutes: Option<u8>,
}

/// `POST /bookings`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。予約する生徒はトークンのアカウント。
pub async fn create_booking<B, J, M, G, A, P, C, K>(
    State(state): Shared<B, J, M, G, A, P, C, K>,
    account: AuthenticatedAccount,
    request: Result<Json<CreateBookingRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<BookingBody>), ApiError>
where
    B: BookingRepository,
    J: LessonMenuLookup,
    M: TrialUsageLookup,
    G: QuotaGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
    K: BookingCoachDisplayNameLookup,
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
        .map_err(ApiError::from)?;
    let booking = describe(&state.describe_booking, booking).await?;
    Ok((StatusCode::CREATED, Json(booking)))
}

/// `POST /bookings/{booking_id}/confirm`。生徒だけが呼べ、確定できるのは自分の予約だけ。
/// 月額の生徒のビデオ通話の暦月の枠が残っていなければ 409 `live_lesson_quota_exceeded`（不変条件 1）。失敗の応答は
/// [`crate::scheduling_error`] が決める。
pub async fn confirm_booking<B, J, M, G, A, P, C, K>(
    State(state): Shared<B, J, M, G, A, P, C, K>,
    account: AuthenticatedAccount,
    Path(booking_id): Path<String>,
) -> Result<Json<BookingBody>, ApiError>
where
    B: BookingRepository,
    J: LessonMenuLookup,
    M: TrialUsageLookup,
    G: QuotaGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
    K: BookingCoachDisplayNameLookup,
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
        .map_err(ApiError::from)?;
    Ok(Json(describe(&state.describe_booking, booking).await?))
}

/// `POST /bookings/{booking_id}/cancel`。生徒だけが呼べ、キャンセルできるのは自分の予約だけ。
///
/// 回数を消費するかはレスポンスに含めない（契約。判定は境界間イベント `BookingCancelled` の
/// `consumes_quota` で entitlement に渡す）。リクエストの本文（任意の `reason`）は読まない。
pub async fn cancel_booking<B, J, M, G, A, P, C, K>(
    State(state): Shared<B, J, M, G, A, P, C, K>,
    account: AuthenticatedAccount,
    Path(booking_id): Path<String>,
) -> Result<Json<BookingBody>, ApiError>
where
    B: BookingRepository,
    J: LessonMenuLookup,
    M: TrialUsageLookup,
    G: QuotaGate,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
    K: BookingCoachDisplayNameLookup,
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
        .map_err(ApiError::from)?;
    Ok(Json(describe(&state.describe_booking, booking).await?))
}

/// 作った・確定した・キャンセルした予約に、スロットとコーチの表示名を添えた student-app の `Booking`。
async fn describe<B, K>(
    describe_booking: &DescribeBooking<B, K>,
    booking: Booking,
) -> Result<BookingBody, ApiError>
where
    B: BookingRepository,
    K: BookingCoachDisplayNameLookup,
{
    let described = describe_booking
        .execute(booking)
        .await
        .map_err(ApiError::internal)?;
    let DescribedBooking {
        booking,
        slot,
        coach_display_name,
    } = described;
    student_app_booking_body(&booking, &slot, coach_display_name.as_ref())
}

/// student-app の `Booking`（予約を返す操作の予約）。admin-web と同じ項目に、予約したスロット `slot` の終了時刻・スポーツと
/// コーチの表示名を足す。表示名は `coach_display_name` が `None`（コーチのアカウントが引けない）なら載せない。
pub(crate) fn student_app_booking_body(
    booking: &Booking,
    slot: &Slot,
    coach_display_name: Option<&DisplayName>,
) -> Result<BookingBody, ApiError> {
    Ok(BookingBody {
        ends_at: Some(date_time_to_contract(slot.ends_at())?),
        sport: Some(sport_to_contract(slot.sport())),
        coach_display_name: coach_display_name.map(|name| name.as_str().to_string()),
        ..booking_body(booking)?
    })
}

/// admin-web の `Booking`。student-app だけの項目（`ends_at`・`sport`・`coach_display_name`）は載せない。
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
        ends_at: None,
        sport: None,
        coach_display_name: None,
    })
}

pub(crate) fn booking_lesson_menu_body(menu: &BookingLessonMenu) -> BookingLessonMenuBody {
    BookingLessonMenuBody {
        lesson_menu_id: lesson_menu_id_uuid::format(menu.id()),
        name: menu.name().as_str().to_string(),
        duration_minutes: menu.duration().map(|d| d.minutes()),
    }
}

/// 生徒の予約の作成・確定・キャンセル（student-app の `POST /bookings`・`POST /bookings/{booking_id}/confirm`・
/// `POST /bookings/{booking_id}/cancel`）のルーター。組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// どれも生徒だけ。同じパスの `GET /bookings`（本部が見る予約の一覧）は [`crate::headquarters_bookings_router`] にあり、
/// `merge` で 1 つのパスにまとまる。
pub fn bookings_router<B, J, M, G, A, P, C, K, T>(
    create_booking: CreateBooking<B, J, M, G, A, C>,
    confirm_booking: ConfirmBooking<B, G, A, P, C>,
    cancel_booking: CancelBooking<B, A, P, C>,
    describe_booking: DescribeBooking<B, K>,
    tokens: T,
) -> Router
where
    B: BookingRepository + 'static,
    J: LessonMenuLookup + 'static,
    M: TrialUsageLookup + 'static,
    G: QuotaGate + 'static,
    A: AnalyticsSink + 'static,
    P: DomainEventPublisher + 'static,
    C: Clock + 'static,
    K: BookingCoachDisplayNameLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/bookings",
            post(self::create_booking::<B, J, M, G, A, P, C, K>),
        )
        .route(
            "/bookings/{booking_id}/confirm",
            post(self::confirm_booking::<B, J, M, G, A, P, C, K>),
        )
        .route(
            "/bookings/{booking_id}/cancel",
            post(self::cancel_booking::<B, J, M, G, A, P, C, K>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(BookingsState {
            create_booking,
            confirm_booking,
            cancel_booking,
            describe_booking,
        }))
        .layer(TokenVerification::layer(tokens))
}
