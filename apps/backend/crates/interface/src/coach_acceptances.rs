//! `POST /coaches/{coach_id}/suspend`・`POST /coaches/{coach_id}/resume`。admin-web の `suspendCoach`・`resumeCoach`
//! （本部がコーチの受付を止める・再開する）。本部だけが呼べる。

use std::sync::Arc;

use application::{
    identity::account_id_uuid,
    scheduling::{CoachAcceptanceRepository, CoachIntroductionLookup, ResumeCoach, SuspendCoach},
};
use axum::{
    Json,
    extract::{Path, State},
};
use domain::{identity::AccountId, scheduling::CoachAcceptance};
use serde::Serialize;

use crate::{
    contract::coach_acceptance_status_to_contract, error::ApiError,
    scheduling_error::coach_not_found,
};

/// `/coaches/{coach_id}/suspend`・`/resume` のハンドラーが使うユースケース。
pub struct CoachAcceptancesState<R, I> {
    pub suspend_coach: SuspendCoach<R, I>,
    pub resume_coach: ResumeCoach<R, I>,
}

/// admin-web の `CoachAcceptance`。
#[derive(Serialize)]
pub struct CoachAcceptanceBody {
    coach_id: String,
    acceptance_status: &'static str,
}

/// `POST /coaches/{coach_id}/suspend`。本部だけが呼べ（役割は `require_role` の層が確かめる）、停止中にした受付を返す。
/// リクエストの本文は読まない。失敗の応答は [`crate::scheduling_error`] が決める。
pub async fn suspend_coach<R, I>(
    State(state): State<Arc<CoachAcceptancesState<R, I>>>,
    Path(coach_id): Path<String>,
) -> Result<Json<CoachAcceptanceBody>, ApiError>
where
    R: CoachAcceptanceRepository,
    I: CoachIntroductionLookup,
{
    let coach_id = parse_coach_id(&coach_id)?;
    let acceptance = state.suspend_coach.execute(coach_id).await?;
    Ok(Json(coach_acceptance_body(&acceptance)))
}

/// `POST /coaches/{coach_id}/resume`。本部だけが呼べ、受付中に戻した受付を返す。リクエストの本文は読まない。
pub async fn resume_coach<R, I>(
    State(state): State<Arc<CoachAcceptancesState<R, I>>>,
    Path(coach_id): Path<String>,
) -> Result<Json<CoachAcceptanceBody>, ApiError>
where
    R: CoachAcceptanceRepository,
    I: CoachIntroductionLookup,
{
    let coach_id = parse_coach_id(&coach_id)?;
    let acceptance = state.resume_coach.execute(coach_id).await?;
    Ok(Json(coach_acceptance_body(&acceptance)))
}

/// 契約の両操作に 422 はないので、UUID でない ID はいないコーチとして扱う。
fn parse_coach_id(coach_id: &str) -> Result<AccountId, ApiError> {
    account_id_uuid::parse(coach_id).map_err(|_| coach_not_found())
}

fn coach_acceptance_body(acceptance: &CoachAcceptance) -> CoachAcceptanceBody {
    CoachAcceptanceBody {
        coach_id: account_id_uuid::format(acceptance.coach_id()),
        acceptance_status: coach_acceptance_status_to_contract(acceptance.status()),
    }
}
