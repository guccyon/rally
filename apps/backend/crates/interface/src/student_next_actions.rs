//! `/me/next-actions`。student-app の `GET /me/next-actions`（`getMyNextActions`。自分の次の操作。ADR 0010）。
//! 生徒だけが呼べる。

use std::sync::Arc;

use application::{
    async_lesson::{VideoRepository, video_id_uuid},
    clock::Clock,
    entitlement::EntitlementRepository,
    identity::TokenVerifier,
    lesson::LessonCompletionRepository,
    live_lesson::{LiveLessonSessionRepository, session_id_uuid},
    scheduling::{
        BookingCoachDisplayNameLookup, BookingRepository, GetMyNextActions, MyNextActions,
        booking_id_uuid,
    },
};
use axum::{Json, Router, extract::State, routing::get};
use domain::{
    identity::{AccountId, Role},
    lesson::LessonType,
    scheduling::{Booking, NextAction, Slot},
};
use serde::Serialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    bookings::{BookingLessonMenuBody, booking_lesson_menu_body},
    contract::{date_time_to_contract, lesson_type_to_contract, sport_to_contract},
    error::ApiError,
};

/// `/me/next-actions` のハンドラーが使うユースケース。
pub struct StudentNextActionsState<B, L, W, R, E, D, C> {
    pub get_my_next_actions: GetMyNextActions<B, L, W, R, E, D, C>,
}

/// ハンドラーが共有する [`StudentNextActionsState`]。
type Shared<B, L, W, R, E, D, C> = Arc<StudentNextActionsState<B, L, W, R, E, D, C>>;

/// student-app の `NextActionList`。
#[derive(Serialize)]
pub struct NextActionListBody {
    next_actions: Vec<NextActionBody>,
}

/// student-app の `NextAction`。`kind` で使わない項目は載せない（`None`）。`lesson_menu`・`coach_display_name` は
/// 載せる `kind` では値がなければ null（`Some(None)`）。
#[derive(Serialize, Default)]
pub struct NextActionBody {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    booking_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_trial: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    starts_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ends_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sport: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lesson_menu: Option<Option<BookingLessonMenuBody>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lesson_type: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    submitted_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    returned_at: Option<String>,
}

/// `GET /me/next-actions`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンのアカウントの
/// 次の操作。
pub async fn get_my_next_actions<B, L, W, R, E, D, C>(
    State(state): State<Shared<B, L, W, R, E, D, C>>,
    account: AuthenticatedAccount,
) -> Result<Json<NextActionListBody>, ApiError>
where
    B: BookingRepository,
    L: LiveLessonSessionRepository,
    W: VideoRepository,
    R: LessonCompletionRepository,
    E: EntitlementRepository,
    D: BookingCoachDisplayNameLookup,
    C: Clock,
{
    let next_actions = state
        .get_my_next_actions
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(NextActionListBody {
        next_actions: next_actions
            .actions
            .iter()
            .map(|action| next_action_body(action, &next_actions))
            .collect::<Result<_, ApiError>>()?,
    }))
}

fn next_action_body(action: &NextAction, list: &MyNextActions) -> Result<NextActionBody, ApiError> {
    let coach_display_name = |coach_id: AccountId| {
        Some(
            list.coaches
                .get(&coach_id)
                .map(|name| name.as_str().to_string()),
        )
    };
    Ok(match action {
        NextAction::ConfirmBooking { booking, slot } => NextActionBody {
            kind: "confirm_booking",
            ..booking_fields(booking, slot, coach_display_name(booking.coach_id()))?
        },
        NextAction::JoinLiveLesson { booking, slot } => NextActionBody {
            kind: "join_live_lesson",
            ..booking_fields(booking, slot, coach_display_name(booking.coach_id()))?
        },
        NextAction::RecordLiveLessonCompletion { session, slot } => NextActionBody {
            kind: "record_lesson_completion",
            lesson_type: Some(lesson_type_to_contract(LessonType::Live)),
            coach_display_name: coach_display_name(session.coach_id()),
            session_id: Some(session_id_uuid::format(session.id())),
            starts_at: Some(date_time_to_contract(slot.starts_at())?),
            ends_at: Some(date_time_to_contract(slot.ends_at())?),
            ..Default::default()
        },
        NextAction::RecordAsyncLessonCompletion { video } => NextActionBody {
            kind: "record_lesson_completion",
            lesson_type: Some(lesson_type_to_contract(LessonType::Async)),
            coach_display_name: coach_display_name(video.coach_id()),
            video_id: Some(video_id_uuid::format(video.id())),
            returned_at: video
                .review()
                .map(|review| date_time_to_contract(review.returned_at()))
                .transpose()?,
            ..Default::default()
        },
        NextAction::VideoReviewInProgress { video } => NextActionBody {
            kind: "video_review_in_progress",
            video_id: Some(video_id_uuid::format(video.id())),
            submitted_at: Some(date_time_to_contract(video.submitted_at())?),
            coach_display_name: coach_display_name(video.coach_id()),
            ..Default::default()
        },
        NextAction::StartSubscription => NextActionBody {
            kind: "start_subscription",
            ..Default::default()
        },
    })
}

/// 予約の次の操作（`confirm_booking`・`join_live_lesson`）に載る項目。
fn booking_fields(
    booking: &Booking,
    slot: &Slot,
    coach_display_name: Option<Option<String>>,
) -> Result<NextActionBody, ApiError> {
    Ok(NextActionBody {
        booking_id: Some(booking_id_uuid::format(booking.id())),
        is_trial: Some(booking.is_trial()),
        starts_at: Some(date_time_to_contract(slot.starts_at())?),
        ends_at: Some(date_time_to_contract(slot.ends_at())?),
        sport: Some(sport_to_contract(slot.sport())),
        lesson_menu: Some(booking.lesson_menu().map(booking_lesson_menu_body)),
        coach_display_name,
        ..Default::default()
    })
}

/// 生徒の次の操作（student-app の `GET /me/next-actions`）のルーター。組み立て用のクレートは他の機能のルーターと
/// `merge` する（同じ検証器を渡す）。
///
/// `GET /me/next-actions` は生徒だけ（コーチ・本部は 403）。
pub fn student_next_actions_router<B, L, W, R, E, D, C, T>(
    get_my_next_actions: GetMyNextActions<B, L, W, R, E, D, C>,
    tokens: T,
) -> Router
where
    B: BookingRepository + 'static,
    L: LiveLessonSessionRepository + 'static,
    W: VideoRepository + 'static,
    R: LessonCompletionRepository + 'static,
    E: EntitlementRepository + 'static,
    D: BookingCoachDisplayNameLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/me/next-actions",
            get(self::get_my_next_actions::<B, L, W, R, E, D, C>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(StudentNextActionsState {
            get_my_next_actions,
        }))
        .layer(TokenVerification::layer(tokens))
}
