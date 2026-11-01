//! `POST /coaches/{coach_id}/suspend`・`POST /coaches/{coach_id}/resume`。admin-web の `suspendCoach`・`resumeCoach`
//! （本部がコーチの受付を止める・再開する）。本部だけが呼べる。止めた・再開した記録の照会（`GET
//! /coaches/{coach_id}/acceptance-changes`）は [`crate::coach_acceptance_changes`]。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    scheduling::{
        CoachAcceptanceRepository, CoachIntroductionLookup, ListCoachAcceptanceChanges,
        ResumeCoach, SuspendCoach,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use domain::{
    identity::{AccountId, Role},
    scheduling::CoachAcceptance,
};
use serde::Serialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    coach_acceptance_changes,
    contract::coach_acceptance_status_to_contract,
    error::ApiError,
    scheduling_error::coach_not_found,
};

/// `/coaches/{coach_id}/suspend`・`/resume`・`/acceptance-changes` のハンドラーが使うユースケース。
pub struct CoachAcceptancesState<R, I, C> {
    pub suspend_coach: SuspendCoach<R, I, C>,
    pub resume_coach: ResumeCoach<R, I, C>,
    pub list_coach_acceptance_changes: ListCoachAcceptanceChanges<R, I>,
}

/// admin-web の `CoachAcceptance`。
#[derive(Serialize)]
pub struct CoachAcceptanceBody {
    coach_id: String,
    acceptance_status: &'static str,
}

/// `POST /coaches/{coach_id}/suspend`。本部だけが呼べ（役割は `require_role` の層が確かめる）、停止中にした受付を返す。
/// 止めた記録には、トークンのアカウントを操作した本部として残す。リクエストの本文は読まない。失敗の応答は [`crate::scheduling_error`] が決める。
pub async fn suspend_coach<R, I, C>(
    State(state): State<Arc<CoachAcceptancesState<R, I, C>>>,
    account: AuthenticatedAccount,
    Path(coach_id): Path<String>,
) -> Result<Json<CoachAcceptanceBody>, ApiError>
where
    R: CoachAcceptanceRepository,
    I: CoachIntroductionLookup,
    C: Clock,
{
    let coach_id = parse_coach_id(&coach_id)?;
    let acceptance = state
        .suspend_coach
        .execute(coach_id, account.account_id())
        .await?;
    Ok(Json(coach_acceptance_body(&acceptance)))
}

/// `POST /coaches/{coach_id}/resume`。本部だけが呼べ、受付中に戻した受付を返す。リクエストの本文は読まない。
pub async fn resume_coach<R, I, C>(
    State(state): State<Arc<CoachAcceptancesState<R, I, C>>>,
    account: AuthenticatedAccount,
    Path(coach_id): Path<String>,
) -> Result<Json<CoachAcceptanceBody>, ApiError>
where
    R: CoachAcceptanceRepository,
    I: CoachIntroductionLookup,
    C: Clock,
{
    let coach_id = parse_coach_id(&coach_id)?;
    let acceptance = state
        .resume_coach
        .execute(coach_id, account.account_id())
        .await?;
    Ok(Json(coach_acceptance_body(&acceptance)))
}

/// 契約の 3 つの操作に 422 はないので、UUID でない ID はいないコーチとして扱う。
pub(crate) fn parse_coach_id(coach_id: &str) -> Result<AccountId, ApiError> {
    account_id_uuid::parse(coach_id).map_err(|_| coach_not_found())
}

fn coach_acceptance_body(acceptance: &CoachAcceptance) -> CoachAcceptanceBody {
    CoachAcceptanceBody {
        coach_id: account_id_uuid::format(acceptance.coach_id()),
        acceptance_status: coach_acceptance_status_to_contract(acceptance.status()),
    }
}

/// 本部によるコーチの受付の停止・再開と、止めた・再開した記録の照会（admin-web の `POST /coaches/{coach_id}/suspend`・
/// `POST /coaches/{coach_id}/resume`・`GET /coaches/{coach_id}/acceptance-changes`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// どれも本部だけ（コーチ・生徒は 403）。
pub fn coach_acceptance_router<R, I, C, T>(
    suspend_coach: SuspendCoach<R, I, C>,
    resume_coach: ResumeCoach<R, I, C>,
    list_coach_acceptance_changes: ListCoachAcceptanceChanges<R, I>,
    tokens: T,
) -> Router
where
    R: CoachAcceptanceRepository + 'static,
    I: CoachIntroductionLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/{coach_id}/suspend",
            post(self::suspend_coach::<R, I, C>),
        )
        .route(
            "/coaches/{coach_id}/resume",
            post(self::resume_coach::<R, I, C>),
        )
        .route(
            "/coaches/{coach_id}/acceptance-changes",
            get(coach_acceptance_changes::list_coach_acceptance_changes::<R, I, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(CoachAcceptancesState {
            suspend_coach,
            resume_coach,
            list_coach_acceptance_changes,
        }))
        .layer(TokenVerification::layer(tokens))
}
