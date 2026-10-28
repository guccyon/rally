//! `/slots`。student-app の `GET /slots`（`searchSlots`）と、admin-web のコーチ用の
//! `POST /slots`（`publishSlot`）。

use std::{num::NonZeroUsize, sync::Arc};

use application::{
    analytics::AnalyticsSink,
    clock::Clock,
    identity::account_id_uuid,
    scheduling::{
        CoachIntroduction, CoachIntroductionLookup, PublishSlot, PublishSlotCommand,
        PublishSlotError, SearchSlots, SearchSlotsQuery, SlotRepository, slot_id_uuid,
    },
};
use axum::{
    Json,
    extract::{
        Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
};
use domain::{
    DomainError,
    scheduling::{Slot, SlotCapacity},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedAccount,
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
pub struct SlotsState<S, I, A, C> {
    pub publish_slot: PublishSlot<S, A, C>,
    pub search_slots: SearchSlots<S, I, C>,
}

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
}

/// admin-web の `PublishSlotRequest`。`capacity` は任意（省けば定員 1。範囲の検証は domain の [`SlotCapacity`]）。
#[derive(Deserialize)]
pub struct PublishSlotRequest {
    sport: String,
    starts_at: String,
    ends_at: String,
    capacity: Option<i64>,
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
}

/// `GET /slots`。認証した利用者なら誰でも呼べる。
pub async fn search_slots<S, I, A, C>(
    State(state): State<Arc<SlotsState<S, I, A, C>>>,
    _account: AuthenticatedAccount,
    params: Result<Query<SearchSlotsParams>, QueryRejection>,
) -> Result<Json<SlotPageBody>, ApiError>
where
    S: SlotRepository,
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
            .map(|slot| student_app_slot_body(slot, page.coaches.get(&slot.coach_id())))
            .collect::<Result<_, _>>()?,
        next_cursor: page.next_cursor.map(slot_cursor_to_contract),
    }))
}

/// `POST /slots`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。
pub async fn publish_slot<S, I, A, C>(
    State(state): State<Arc<SlotsState<S, I, A, C>>>,
    account: AuthenticatedAccount,
    request: Result<Json<PublishSlotRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AdminWebSlotBody>), ApiError>
where
    S: SlotRepository,
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
    };

    let slot = state
        .publish_slot
        .execute(command)
        .await
        .map_err(|error| match error {
            PublishSlotError::Invalid(error @ DomainError::SlotOverlaps { .. }) => {
                ApiError::conflict("slot_overlaps", crate::message::domain_error(&error))
            }
            PublishSlotError::Invalid(error) => ApiError::from(error),
            PublishSlotError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok((StatusCode::CREATED, Json(admin_web_slot_body(&slot)?)))
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

pub(crate) fn student_app_slot_body(
    slot: &Slot,
    coach: Option<&CoachIntroduction>,
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
    })
}

pub(crate) fn admin_web_slot_body(slot: &Slot) -> Result<AdminWebSlotBody, ApiError> {
    Ok(AdminWebSlotBody {
        slot_id: slot_id_uuid::format(slot.id()),
        coach_id: account_id_uuid::format(slot.coach_id()),
        sport: sport_to_contract(slot.sport()),
        starts_at: date_time_to_contract(slot.starts_at())?,
        ends_at: date_time_to_contract(slot.ends_at())?,
        status: slot_status_to_contract(slot.status()),
        capacity: slot.capacity().value(),
        remaining_seats: slot.remaining_seats(),
    })
}
