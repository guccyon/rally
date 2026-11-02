//! `GET /bookings`。admin-web の `listBookings`（本部が見る予約の一覧）。本部だけが呼べる。同じパスの student-app の
//! `POST /bookings`（予約の作成。生徒だけ）は [`crate::bookings`]。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    scheduling::{
        BookingCoachDisplayNameLookup, BookingRepository, ListBookings, ListBookingsQuery,
        StudentDisplayNameLookup,
    },
};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use domain::{identity::Role, scheduling::BookingStatus};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    bookings::{BookingBody, booking_body},
    contract::{booking_status_from_contract, date_from_contract, date_to_contract},
    error::ApiError,
};

/// `GET /bookings`（本部）のハンドラーが使うユースケース。
pub struct HeadquartersBookingsState<B, K, D, C> {
    pub list_bookings: ListBookings<B, K, D, C>,
}

/// admin-web の `listBookings` のクエリ（繰り返せる `status` は除く。[`statuses_from_query`] が読む）。
#[derive(Deserialize)]
pub struct ListBookingsParams {
    from: Option<String>,
    to: Option<String>,
    coach_id: Option<String>,
    student_id: Option<String>,
    trial: Option<bool>,
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

/// `listBookings` の `status`（`status=pending&status=cancelled` のように繰り返せる）を読む。重ねて付けた同じ値は 1 つに
/// まとめる。契約の `BookingStatus` にない値は 422。
fn statuses_from_query(pairs: &[(String, String)]) -> Result<Vec<BookingStatus>, ApiError> {
    let mut statuses = Vec::new();
    for (_, value) in pairs.iter().filter(|(key, _)| key == "status") {
        let status = booking_status_from_contract(value)?;
        if !statuses.contains(&status) {
            statuses.push(status);
        }
    }
    Ok(statuses)
}

/// `GET /bookings`。本部だけが呼べる（役割は `require_role` の層が確かめる）。`from`・`to` が `YYYY-MM-DD` の日付で
/// ないとき・`coach_id`・`student_id` が UUID でないとき・`status` が予約の状態でないとき・`trial` が `true` / `false` でないときと、
/// 期間の違反は 422（失敗の応答は
/// [`crate::scheduling_error`] が決める）。
pub async fn list_bookings<B, K, D, C>(
    State(state): State<Arc<HeadquartersBookingsState<B, K, D, C>>>,
    _account: AuthenticatedAccount,
    params: Result<Query<ListBookingsParams>, QueryRejection>,
    pairs: Result<Query<Vec<(String, String)>>, QueryRejection>,
) -> Result<Json<HeadquartersBookingListBody>, ApiError>
where
    B: BookingRepository,
    K: BookingCoachDisplayNameLookup,
    D: StudentDisplayNameLookup,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let Query(pairs) =
        pairs.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
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
        coach_id: params
            .coach_id
            .as_deref()
            .map(|id| {
                account_id_uuid::parse(id)
                    .map_err(|_| ApiError::validation_failed("coach_id が UUID でない"))
            })
            .transpose()?,
        student_id: params
            .student_id
            .as_deref()
            .map(|id| {
                account_id_uuid::parse(id)
                    .map_err(|_| ApiError::validation_failed("student_id が UUID でない"))
            })
            .transpose()?,
        statuses: statuses_from_query(&pairs)?,
        trial: params.trial,
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

/// 本部が見る予約の一覧（admin-web の `GET /bookings`）のルーター。組み立て用のクレートは他の機能のルーターと `merge`
/// する（同じ検証器を渡す）。同じパスの `POST /bookings`（student-app の予約の作成。生徒だけ）は
/// [`crate::bookings_router`] にあり、`merge` で 1 つのパスにまとまる。
///
/// `GET /bookings` は本部だけ（コーチ・生徒は 403）。
pub fn headquarters_bookings_router<B, K, D, C, T>(
    list_bookings: ListBookings<B, K, D, C>,
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
        .route("/bookings", get(self::list_bookings::<B, K, D, C>))
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(HeadquartersBookingsState { list_bookings }))
        .layer(TokenVerification::layer(tokens))
}
