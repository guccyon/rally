//! `/coaches/me/bookings`。admin-web の `GET /coaches/me/bookings`（`listMyCoachBookings`。コーチの自分の予約の一覧）。
//! コーチだけが呼べる。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::TokenVerifier,
    live_lesson::{LiveLessonSessionRepository, session_id_uuid},
    scheduling::{
        BookingRepository, ListCoachBookings, ListCoachBookingsQuery, StudentDisplayNameLookup,
    },
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::{identity::Role, live_lesson::LiveLessonSession};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    bookings::{BookingBody, booking_body},
    contract::{
        date_from_contract, date_time_to_contract, date_to_contract, session_status_to_contract,
    },
    error::ApiError,
    slots::{AdminWebSlotBody, admin_web_slot_body},
};

/// `/coaches/me/bookings` のハンドラーが使うユースケース。
pub struct CoachBookingsState<B, L, D, C> {
    pub list_coach_bookings: ListCoachBookings<B, L, D, C>,
}

/// admin-web の `listMyCoachBookings` のクエリ。
#[derive(Deserialize)]
pub struct ListMyCoachBookingsParams {
    from: Option<String>,
    to: Option<String>,
}

/// admin-web の `CoachBookingList`。期間は `from`・`to` のどちらかを付けたときだけ載せる。
#[derive(Serialize)]
pub struct CoachBookingListBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    bookings: Vec<CoachBookingBody>,
}

/// admin-web の `CoachBooking`。生徒の表示名は任意の項目で、引けなければ載せない。通話中のセッションは、予約に結び付いた
/// 通話中のセッションがあるときだけ載せる。
#[derive(Serialize)]
pub struct CoachBookingBody {
    booking: BookingBody,
    slot: AdminWebSlotBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    live_lesson_session: Option<CoachBookingLiveLessonSessionBody>,
}

/// admin-web の `CoachBookingLiveLessonSession`（予約の通話中のセッションの要約）。
#[derive(Serialize)]
pub struct CoachBookingLiveLessonSessionBody {
    session_id: String,
    status: &'static str,
    started_at: String,
}

fn live_lesson_session_body(
    session: &LiveLessonSession,
) -> Result<CoachBookingLiveLessonSessionBody, ApiError> {
    Ok(CoachBookingLiveLessonSessionBody {
        session_id: session_id_uuid::format(session.id()),
        status: session_status_to_contract(session.status()),
        started_at: date_time_to_contract(session.started_at())?,
    })
}

/// `GET /coaches/me/bookings`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンの
/// アカウントのスロットへの予約。`from`・`to` が `YYYY-MM-DD` の日付でないときと、期間の違反は 422（失敗の応答は
/// [`crate::scheduling_error`] が決める。本部の `GET /bookings` と同じ）。
pub async fn list_my_coach_bookings<B, L, D, C>(
    State(state): State<Arc<CoachBookingsState<B, L, D, C>>>,
    account: AuthenticatedAccount,
    params: Result<Query<ListMyCoachBookingsParams>, QueryRejection>,
) -> Result<Json<CoachBookingListBody>, ApiError>
where
    B: BookingRepository,
    L: LiveLessonSessionRepository,
    D: StudentDisplayNameLookup,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let query = ListCoachBookingsQuery {
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
        .list_coach_bookings
        .execute(account.account_id(), query)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(CoachBookingListBody {
        from: list.from.map(date_to_contract),
        to: list.to.map(date_to_contract),
        bookings: list
            .items
            .iter()
            .map(|(booking, slot)| {
                Ok(CoachBookingBody {
                    booking: booking_body(booking)?,
                    slot: admin_web_slot_body(slot, None)?,
                    student_display_name: list
                        .students
                        .get(&booking.student_id())
                        .map(|name| name.as_str().to_string()),
                    live_lesson_session: list
                        .sessions
                        .get(&booking.id())
                        .map(live_lesson_session_body)
                        .transpose()?,
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}

/// コーチの自分の予約の一覧（admin-web の `GET /coaches/me/bookings`）のルーター。組み立て用のクレートは他の機能の
/// ルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /coaches/me/bookings` はコーチだけ。
pub fn coach_bookings_router<B, L, D, C, T>(
    list_coach_bookings: ListCoachBookings<B, L, D, C>,
    tokens: T,
) -> Router
where
    B: BookingRepository + 'static,
    L: LiveLessonSessionRepository + 'static,
    D: StudentDisplayNameLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/me/bookings",
            get(list_my_coach_bookings::<B, L, D, C>),
        )
        .route_layer(require_role(&[Role::Coach]))
        .with_state(Arc::new(CoachBookingsState {
            list_coach_bookings,
        }))
        .layer(TokenVerification::layer(tokens))
}
