//! `/slots`。student-app の `GET /slots`（`searchSlots`）と、admin-web のコーチ用の
//! `POST /slots`（`publishSlot`）。

use std::{collections::BTreeMap, num::NonZeroUsize, sync::Arc};

use application::{
    analytics::AnalyticsSink,
    catalog::lesson_menu_id_uuid,
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    scheduling::{
        CoachIntroduction, CoachIntroductionLookup, LessonMenuLookup, PublishSlot,
        PublishSlotCommand, SearchSlots, SearchSlotsQuery, SlotRepository, slot_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{
        Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    routing::{get, post},
};
use domain::{
    catalog::{LessonMenu, LessonMenuId},
    identity::Role,
    scheduling::{Slot, SlotCapacity},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::{
        admin_web_sport, date_time_from_contract, date_time_to_contract, slot_cursor_from_contract,
        slot_cursor_to_contract, slot_status_to_contract, sport_to_contract, student_app_sport,
    },
    error::ApiError,
};

/// `limit` の既定値と上限（student-app の `searchSlots`）。
const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

/// `/slots` のハンドラーが使うユースケース。
pub struct SlotsState<S, J, I, A, C> {
    pub publish_slot: PublishSlot<S, J, A, C>,
    pub search_slots: SearchSlots<S, J, I, C>,
}

/// ハンドラーが共有する [`SlotsState`]。
type Shared<S, J, I, A, C> = Arc<SlotsState<S, J, I, A, C>>;

/// student-app の `searchSlots` のクエリ。値の検証はハンドラーで行う。
#[derive(Deserialize)]
pub struct SearchSlotsParams {
    sport: String,
    coach_id: Option<String>,
    starts_after: Option<String>,
    starts_before: Option<String>,
    limit: Option<String>,
    cursor: Option<String>,
}

/// student-app の `SlotPage`。
#[derive(Serialize)]
pub struct SlotPageBody {
    items: Vec<StudentAppSlotBody>,
    next_cursor: Option<String>,
}

/// student-app の `Slot`。コーチの紹介（表示名・自己紹介文）は任意の項目で、引けなければ載せない。
#[derive(Serialize)]
pub struct StudentAppSlotBody {
    slot_id: String,
    coach_id: String,
    sport: &'static str,
    starts_at: String,
    ends_at: String,
    capacity: u8,
    remaining_seats: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    /// 自己紹介文を書いていない（空の）コーチは載せない
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_bio: Option<String>,
    /// 結びついたレッスンメニュー。外側の `None` は項目を載せない（メニューを添えない操作）、内側の `None` は null
    #[serde(skip_serializing_if = "Option::is_none")]
    lesson_menu: Option<Option<SlotLessonMenuBody>>,
}

/// student-app・admin-web の `SlotLessonMenu`（枠に結びついたレッスンメニューの ID・名前・1 回の時間）。
#[derive(Serialize)]
pub struct SlotLessonMenuBody {
    lesson_menu_id: String,
    name: String,
    duration_minutes: Option<u8>,
}

/// admin-web の `PublishSlotRequest`。`capacity` は任意（省けば定員 1。範囲の検証は domain の [`SlotCapacity`]）。
#[derive(Deserialize)]
pub struct PublishSlotRequest {
    sport: String,
    starts_at: String,
    ends_at: String,
    capacity: Option<i64>,
    lesson_menu_id: Option<String>,
}

/// admin-web の `Slot`。
#[derive(Serialize)]
pub struct AdminWebSlotBody {
    slot_id: String,
    coach_id: String,
    sport: &'static str,
    starts_at: String,
    ends_at: String,
    status: &'static str,
    capacity: u8,
    remaining_seats: u32,
    /// 結びついたレッスンメニュー（[`StudentAppSlotBody`] の `lesson_menu` と同じ）
    #[serde(skip_serializing_if = "Option::is_none")]
    lesson_menu: Option<Option<SlotLessonMenuBody>>,
}

/// `GET /slots`。認証した利用者なら誰でも呼べる。
pub async fn search_slots<S, J, I, A, C>(
    State(state): State<Shared<S, J, I, A, C>>,
    _account: AuthenticatedAccount,
    params: Result<Query<SearchSlotsParams>, QueryRejection>,
) -> Result<Json<SlotPageBody>, ApiError>
where
    S: SlotRepository,
    J: LessonMenuLookup,
    I: CoachIntroductionLookup,
    A: AnalyticsSink,
    C: Clock,
{
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let query = SearchSlotsQuery {
        sport: student_app_sport(&params.sport)?,
        coach_id: params
            .coach_id
            .as_deref()
            .map(|id| {
                account_id_uuid::parse(id)
                    .map_err(|_| ApiError::validation_failed("coach_id が UUID でない"))
            })
            .transpose()?,
        starts_after: params
            .starts_after
            .as_deref()
            .map(|v| date_time_from_contract("starts_after", v))
            .transpose()?,
        starts_before: params
            .starts_before
            .as_deref()
            .map(|v| date_time_from_contract("starts_before", v))
            .transpose()?,
        limit: limit(params.limit.as_deref())?,
        cursor: params
            .cursor
            .as_deref()
            .map(slot_cursor_from_contract)
            .transpose()?,
    };

    let page = state
        .search_slots
        .execute(query)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(SlotPageBody {
        items: page
            .items
            .iter()
            .map(|slot| {
                student_app_slot_body(
                    slot,
                    page.coaches.get(&slot.coach_id()),
                    Some(&page.lesson_menus),
                )
            })
            .collect::<Result<_, _>>()?,
        next_cursor: page.next_cursor.map(slot_cursor_to_contract),
    }))
}

/// `POST /slots`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。
pub async fn publish_slot<S, J, I, A, C>(
    State(state): State<Shared<S, J, I, A, C>>,
    account: AuthenticatedAccount,
    request: Result<Json<PublishSlotRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AdminWebSlotBody>), ApiError>
where
    S: SlotRepository,
    J: LessonMenuLookup,
    I: CoachIntroductionLookup,
    A: AnalyticsSink,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let command = PublishSlotCommand {
        coach_id: account.account_id(),
        sport: admin_web_sport(&request.sport)?,
        starts_at: date_time_from_contract("starts_at", &request.starts_at)?,
        ends_at: date_time_from_contract("ends_at", &request.ends_at)?,
        capacity: request
            .capacity
            .map(SlotCapacity::new)
            .transpose()?
            .unwrap_or_default(),
        lesson_menu_id: request
            .lesson_menu_id
            .as_deref()
            .map(|id| {
                lesson_menu_id_uuid::parse(id)
                    .map_err(|_| ApiError::validation_failed("lesson_menu_id が UUID でない"))
            })
            .transpose()?,
    };

    let slot = state
        .publish_slot
        .execute(command)
        .await
        .map_err(ApiError::from)?;
    // 公開の応答には結びついたメニュー（`lesson_menu`）を載せない（契約の publishSlot。名前と時間は listMyCoachSlots）
    Ok((StatusCode::CREATED, Json(admin_web_slot_body(&slot, None)?)))
}

/// `limit`（1〜100、既定 20）。
fn limit(value: Option<&str>) -> Result<NonZeroUsize, ApiError> {
    let Some(value) = value else {
        return Ok(NonZeroUsize::new(DEFAULT_LIMIT).expect("既定値は 1 以上"));
    };
    value
        .parse::<usize>()
        .ok()
        .filter(|n| *n <= MAX_LIMIT)
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| ApiError::validation_failed(format!("limit は 1〜{MAX_LIMIT} の整数")))
}

/// `lesson_menus` は枠に結びついたメニュー（[`slot_lesson_menu_body`]）。`None` なら `lesson_menu` を載せない。
pub(crate) fn student_app_slot_body(
    slot: &Slot,
    coach: Option<&CoachIntroduction>,
    lesson_menus: Option<&BTreeMap<LessonMenuId, LessonMenu>>,
) -> Result<StudentAppSlotBody, ApiError> {
    Ok(StudentAppSlotBody {
        slot_id: slot_id_uuid::format(slot.id()),
        coach_id: account_id_uuid::format(slot.coach_id()),
        sport: sport_to_contract(slot.sport()),
        starts_at: date_time_to_contract(slot.starts_at())?,
        ends_at: date_time_to_contract(slot.ends_at())?,
        capacity: slot.capacity().value(),
        remaining_seats: slot.remaining_seats(),
        coach_display_name: coach.map(|coach| coach.display_name.as_str().to_string()),
        coach_bio: coach
            .map(|coach| coach.bio.as_str())
            .filter(|bio| !bio.is_empty())
            .map(str::to_string),
        lesson_menu: lesson_menus.map(|menus| slot_lesson_menu_body(slot, menus)),
    })
}

/// `lesson_menus` は枠に結びついたメニュー（[`slot_lesson_menu_body`]）。`None` なら `lesson_menu` を載せない。
pub(crate) fn admin_web_slot_body(
    slot: &Slot,
    lesson_menus: Option<&BTreeMap<LessonMenuId, LessonMenu>>,
) -> Result<AdminWebSlotBody, ApiError> {
    Ok(AdminWebSlotBody {
        slot_id: slot_id_uuid::format(slot.id()),
        coach_id: account_id_uuid::format(slot.coach_id()),
        sport: sport_to_contract(slot.sport()),
        starts_at: date_time_to_contract(slot.starts_at())?,
        ends_at: date_time_to_contract(slot.ends_at())?,
        status: slot_status_to_contract(slot.status()),
        capacity: slot.capacity().value(),
        remaining_seats: slot.remaining_seats(),
        lesson_menu: lesson_menus.map(|menus| slot_lesson_menu_body(slot, menus)),
    })
}

/// 枠に結びついたメニューの `SlotLessonMenu`。枠にメニューがない、または `lesson_menus` にない（catalog にない・
/// 見せないメニュー）なら `None`（null）。
fn slot_lesson_menu_body(
    slot: &Slot,
    lesson_menus: &BTreeMap<LessonMenuId, LessonMenu>,
) -> Option<SlotLessonMenuBody> {
    let menu = lesson_menus.get(&slot.lesson_menu_id()?)?;
    Some(SlotLessonMenuBody {
        lesson_menu_id: lesson_menu_id_uuid::format(menu.id()),
        name: menu.name().as_str().to_string(),
        duration_minutes: menu.format().duration().map(|d| d.minutes()),
    })
}

/// スロット（`/slots`）のルーター。組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `POST /slots`（admin-web）はコーチだけ、`GET /slots`（student-app）は認証した利用者なら誰でも呼べる。
pub fn slots_router<S, J, I, A, C, T>(
    publish_slot: PublishSlot<S, J, A, C>,
    search_slots: SearchSlots<S, J, I, C>,
    tokens: T,
) -> Router
where
    S: SlotRepository + 'static,
    J: LessonMenuLookup + 'static,
    I: CoachIntroductionLookup + 'static,
    A: AnalyticsSink + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/slots",
            get(self::search_slots::<S, J, I, A, C>).merge(
                post(self::publish_slot::<S, J, I, A, C>).route_layer(require_role(&[Role::Coach])),
            ),
        )
        .with_state(Arc::new(SlotsState {
            publish_slot,
            search_slots,
        }))
        .layer(TokenVerification::layer(tokens))
}
