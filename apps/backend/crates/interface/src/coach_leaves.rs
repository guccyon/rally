//! `GET /coaches/{coach_id}/leave-preview`・`POST /coaches/{coach_id}/leaves`。admin-web の `getCoachLeavePreview`
//! （本部が見る、コーチのお休みの期間の予約と枠）と `startCoachLeave`（本部がコーチをお休みにする。期間の予約をまとめて
//! キャンセルし、生徒に知らせ、枠を取り下げる）。本部だけが呼べる。生徒へのお知らせの文言は [`crate::coach_leave_text`]。

use std::{collections::BTreeSet, sync::Arc};

use application::{
    clock::Clock,
    domain_event::DomainEventPublisher,
    identity::{TokenVerifier, account_id_uuid},
    scheduling::{
        BookingRepository, CoachLeavePreview, CoachLeaveRepository, LessonMenuLookup,
        PreviewCoachLeave, SlotCoachAccountLookup, StartCoachLeave, StartCoachLeaveCommand,
        StudentDisplayNameLookup, booking_id_uuid, calendar_day, coach_leave_id_uuid,
        slot_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    routing::{get, post},
};
use domain::{
    identity::Role,
    scheduling::{CoachLeave, CoachLeaveNote},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    coach_acceptances::parse_coach_id,
    coach_leave_text::{COACH_LEAVE_NOTICE_TITLE, coach_leave_notice_body},
    contract::{
        booking_status_to_contract, coach_acceptance_status_to_contract, date_from_contract,
        date_time_to_contract, date_to_contract, sport_to_contract,
    },
    error::ApiError,
};

/// `/coaches/{coach_id}/leave-preview`・`/coaches/{coach_id}/leaves` のハンドラーが使うユースケース。
pub struct CoachLeavesState<B, A, D, M, P, C> {
    pub preview_coach_leave: PreviewCoachLeave<B, A, D, M, C>,
    pub start_coach_leave: StartCoachLeave<B, A, P, C>,
}

/// ハンドラーが共有する [`CoachLeavesState`]。
type Shared<B, A, D, M, P, C> = Arc<CoachLeavesState<B, A, D, M, P, C>>;

/// admin-web の `getCoachLeavePreview` のクエリ。どちらも必須（省けば 422）。
#[derive(Deserialize)]
pub struct CoachLeavePreviewParams {
    from: Option<String>,
    to: Option<String>,
}

/// admin-web の `CoachLeavePreview`。
#[derive(Serialize)]
pub struct CoachLeavePreviewBody {
    coach: CoachLeaveCoachBody,
    from: String,
    to: String,
    bookings: Vec<CoachLeaveBookingBody>,
    open_slots: Vec<CoachLeaveSlotBody>,
    slot_count: usize,
    student_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    notification_preview: Option<CoachLeaveNotificationPreviewBody>,
}

/// admin-web の `CoachLeaveCoach`。
#[derive(Serialize)]
pub struct CoachLeaveCoachBody {
    coach_id: String,
    display_name: String,
    sports: Vec<&'static str>,
    acceptance_status: &'static str,
}

/// admin-web の `CoachLeaveBooking`。メニューの名前と生徒の表示名は任意の項目で、なければ載せない。
#[derive(Serialize)]
pub struct CoachLeaveBookingBody {
    booking_id: String,
    slot_id: String,
    starts_at: String,
    ends_at: String,
    status: &'static str,
    is_trial: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    lesson_menu_name: Option<String>,
    student_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    student_display_name: Option<String>,
}

/// admin-web の `CoachLeaveSlot`。メニューの名前は任意の項目で、なければ載せない。
#[derive(Serialize)]
pub struct CoachLeaveSlotBody {
    slot_id: String,
    starts_at: String,
    ends_at: String,
    capacity: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    lesson_menu_name: Option<String>,
}

/// admin-web の `CoachLeaveNotificationPreview`。
#[derive(Serialize)]
pub struct CoachLeaveNotificationPreviewBody {
    title: &'static str,
    body: String,
}

/// admin-web の `StartCoachLeaveRequest`。
#[derive(Deserialize)]
pub struct StartCoachLeaveRequest {
    from: String,
    to: String,
    #[serde(default)]
    note: Option<String>,
    expected_booking_ids: Vec<String>,
}

/// admin-web の `CoachLeaveResult`。
#[derive(Serialize)]
pub struct CoachLeaveResultBody {
    coach_leave: CoachLeaveBody,
    cancelled_booking_ids: Vec<String>,
    cancelled_booking_count: usize,
    notified_student_count: usize,
    withdrawn_slot_count: u32,
}

/// admin-web の `CoachLeave`。
#[derive(Serialize)]
pub struct CoachLeaveBody {
    coach_leave_id: String,
    coach_id: String,
    from: String,
    to: String,
    note: String,
    created_by: String,
    created_at: String,
}

/// `GET /coaches/{coach_id}/leave-preview`。本部だけが呼べる（役割は `require_role` の層が確かめる）。`from`・`to` が
/// ない・`YYYY-MM-DD` の日付でない・期間の違反（終わりが始まりより前・31 日を超える・始まりが今日以前）は 422、コーチで
/// ないアカウント・ないアカウント・UUID でない `coach_id` は 404（失敗の応答は [`crate::scheduling_error`] が決める）。
pub async fn get_coach_leave_preview<B, A, D, M, P, C>(
    State(state): State<Shared<B, A, D, M, P, C>>,
    _account: AuthenticatedAccount,
    Path(coach_id): Path<String>,
    params: Result<Query<CoachLeavePreviewParams>, QueryRejection>,
) -> Result<Json<CoachLeavePreviewBody>, ApiError>
where
    B: BookingRepository + CoachLeaveRepository,
    A: SlotCoachAccountLookup,
    D: StudentDisplayNameLookup,
    M: LessonMenuLookup,
    P: DomainEventPublisher,
    C: Clock,
{
    let coach_id = parse_coach_id(&coach_id)?;
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let required = |field: &str, value: Option<String>| {
        value
            .ok_or_else(|| ApiError::validation_failed(format!("{field} がない")))
            .and_then(|value| date_from_contract(field, &value))
    };
    let from = required("from", params.from)?;
    let to = required("to", params.to)?;
    let preview = state
        .preview_coach_leave
        .execute(coach_id, from, to)
        .await?;
    Ok(Json(preview_body(&preview)?))
}

/// `POST /coaches/{coach_id}/leaves`。本部だけが呼べ、お休みにした記録と件数を 201 で返す。操作した本部はトークンの
/// アカウント。`note` は省けば空。日付・ID の形の誤り・期間の違反・201 文字以上の `note`・JSON でない本文は 422、確かめた
/// 後に期間の予約が変わったら 409 `leave_preview_outdated`、期間の重なるお休みがあれば 409 `coach_leave_overlaps`、コーチで
/// ないアカウントは 404（失敗の応答は [`crate::scheduling_error`] が決める）。どの失敗でも何も変えない。
pub async fn start_coach_leave<B, A, D, M, P, C>(
    State(state): State<Shared<B, A, D, M, P, C>>,
    account: AuthenticatedAccount,
    Path(coach_id): Path<String>,
    request: Result<Json<StartCoachLeaveRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CoachLeaveResultBody>), ApiError>
where
    B: BookingRepository + CoachLeaveRepository,
    A: SlotCoachAccountLookup,
    D: StudentDisplayNameLookup,
    M: LessonMenuLookup,
    P: DomainEventPublisher,
    C: Clock,
{
    let coach_id = parse_coach_id(&coach_id)?;
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let expected_booking_ids = request
        .expected_booking_ids
        .iter()
        .map(|id| {
            booking_id_uuid::parse(id).map_err(|_| {
                ApiError::validation_failed(format!("expected_booking_ids の {id} は UUID でない"))
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let command = StartCoachLeaveCommand {
        coach_id,
        from: date_from_contract("from", &request.from)?,
        to: date_from_contract("to", &request.to)?,
        note: CoachLeaveNote::new(request.note.unwrap_or_default())?,
        requested_by: account.account_id(),
        expected_booking_ids,
    };
    let leave = state.start_coach_leave.execute(command).await?;
    Ok((StatusCode::CREATED, Json(result_body(&leave)?)))
}

fn preview_body(preview: &CoachLeavePreview) -> Result<CoachLeavePreviewBody, ApiError> {
    let coach = &preview.coach;
    let targets = &preview.targets;
    let bookings = targets
        .bookings()
        .iter()
        .map(|(booking, slot)| {
            Ok(CoachLeaveBookingBody {
                booking_id: booking_id_uuid::format(booking.id()),
                slot_id: slot_id_uuid::format(slot.id()),
                starts_at: date_time_to_contract(slot.starts_at())?,
                ends_at: date_time_to_contract(slot.ends_at())?,
                status: booking_status_to_contract(booking.status()),
                is_trial: booking.is_trial(),
                lesson_menu_name: booking
                    .lesson_menu()
                    .map(|menu| menu.name().as_str().to_string()),
                student_id: account_id_uuid::format(booking.student_id()),
                student_display_name: preview
                    .students
                    .get(&booking.student_id())
                    .map(|name| name.as_str().to_string()),
            })
        })
        .collect::<Result<_, ApiError>>()?;
    let open_slots = targets
        .open_slots()
        .iter()
        .map(|slot| {
            Ok(CoachLeaveSlotBody {
                slot_id: slot_id_uuid::format(slot.id()),
                starts_at: date_time_to_contract(slot.starts_at())?,
                ends_at: date_time_to_contract(slot.ends_at())?,
                capacity: slot.capacity().value(),
                lesson_menu_name: slot
                    .lesson_menu_id()
                    .and_then(|id| preview.lesson_menus.get(&id))
                    .map(|menu| menu.name().as_str().to_string()),
            })
        })
        .collect::<Result<_, ApiError>>()?;
    let notification_preview =
        targets
            .bookings()
            .first()
            .map(|(_, slot)| CoachLeaveNotificationPreviewBody {
                title: COACH_LEAVE_NOTICE_TITLE,
                body: coach_leave_notice_body(
                    Some(coach.display_name().as_str()),
                    slot.starts_at(),
                    slot.ends_at(),
                ),
            });
    Ok(CoachLeavePreviewBody {
        coach: CoachLeaveCoachBody {
            coach_id: account_id_uuid::format(coach.id()),
            display_name: coach.display_name().as_str().to_string(),
            sports: coach
                .as_coach()
                .map(|coach| coach.sports().iter().copied().map(sport_to_contract).collect())
                .unwrap_or_default(),
            acceptance_status: coach_acceptance_status_to_contract(preview.acceptance_status),
        },
        from: date_to_contract(preview.from),
        to: date_to_contract(preview.to),
        bookings,
        open_slots,
        slot_count: targets.slot_count(),
        student_count: targets.student_count(),
        notification_preview,
    })
}

fn result_body(leave: &CoachLeave) -> Result<CoachLeaveResultBody, ApiError> {
    let (from, to) = calendar_day::dates_of(leave.period());
    Ok(CoachLeaveResultBody {
        coach_leave: CoachLeaveBody {
            coach_leave_id: coach_leave_id_uuid::format(leave.id()),
            coach_id: account_id_uuid::format(leave.coach_id()),
            from: date_to_contract(from),
            to: date_to_contract(to),
            note: leave.note().as_str().to_string(),
            created_by: account_id_uuid::format(leave.created_by()),
            created_at: date_time_to_contract(leave.created_at())?,
        },
        cancelled_booking_ids: leave
            .cancelled_bookings()
            .iter()
            .map(|booking| booking_id_uuid::format(booking.booking_id()))
            .collect(),
        cancelled_booking_count: leave.cancelled_bookings().len(),
        notified_student_count: leave.notified_student_count(),
        withdrawn_slot_count: leave.withdrawn_slot_count(),
    })
}

/// 本部が見るコーチのお休みの期間の予約と枠と、お休みにする操作（admin-web の `GET /coaches/{coach_id}/leave-preview`・
/// `POST /coaches/{coach_id}/leaves`）のルーター。組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を
/// 渡す）。
///
/// どちらも本部だけ（コーチ・生徒は 403）。
pub fn coach_leaves_router<B, A, D, M, P, C, T>(
    preview_coach_leave: PreviewCoachLeave<B, A, D, M, C>,
    start_coach_leave: StartCoachLeave<B, A, P, C>,
    tokens: T,
) -> Router
where
    B: BookingRepository + CoachLeaveRepository + 'static,
    A: SlotCoachAccountLookup + 'static,
    D: StudentDisplayNameLookup + 'static,
    M: LessonMenuLookup + 'static,
    P: DomainEventPublisher + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/{coach_id}/leave-preview",
            get(self::get_coach_leave_preview::<B, A, D, M, P, C>),
        )
        .route(
            "/coaches/{coach_id}/leaves",
            post(self::start_coach_leave::<B, A, D, M, P, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(CoachLeavesState {
            preview_coach_leave,
            start_coach_leave,
        }))
        .layer(TokenVerification::layer(tokens))
}
