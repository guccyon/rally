//! `GET /coaches/{coach_id}/acceptance-changes`。admin-web の `listCoachAcceptanceChanges`（本部がコーチの受付を止めた・
//! 再開した記録をたどる）。本部だけが呼べる。ルーターと状態は停止・再開（[`crate::coach_acceptances`]）と共有する。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::account_id_uuid,
    scheduling::{CoachAcceptanceRepository, CoachIntroductionLookup},
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;

use crate::{
    coach_acceptances::{CoachAcceptancesState, parse_coach_id},
    contract::{coach_acceptance_status_to_contract, date_time_to_contract},
    error::ApiError,
    scheduling_error::coach_not_found,
};

/// admin-web の `CoachAcceptanceChangeList`。
#[derive(Serialize)]
pub struct CoachAcceptanceChangeListBody {
    coach_id: String,
    changes: Vec<CoachAcceptanceChangeBody>,
}

/// admin-web の `CoachAcceptanceChange`。`acceptance_status` は変えた後の状態。`reason` は止めた記録の理由（再開した記録と、
/// 理由を残し始める前の記録は null）。
#[derive(Serialize)]
struct CoachAcceptanceChangeBody {
    acceptance_status: &'static str,
    reason: Option<String>,
    changed_by: String,
    changed_at: String,
}

/// `GET /coaches/{coach_id}/acceptance-changes`。本部だけが呼べ（役割は `require_role` の層が確かめる）、コーチの受付を
/// 止めた・再開した記録を新しい順に返す。コーチでない ID・ない ID・UUID でない ID は 404。
pub async fn list_coach_acceptance_changes<R, I, C>(
    State(state): State<Arc<CoachAcceptancesState<R, I, C>>>,
    Path(coach_id): Path<String>,
) -> Result<Json<CoachAcceptanceChangeListBody>, ApiError>
where
    R: CoachAcceptanceRepository,
    I: CoachIntroductionLookup,
    C: Clock,
{
    let coach_id = parse_coach_id(&coach_id)?;
    let changes = state
        .list_coach_acceptance_changes
        .execute(coach_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(coach_not_found)?;
    Ok(Json(CoachAcceptanceChangeListBody {
        coach_id: account_id_uuid::format(coach_id),
        changes: changes
            .iter()
            .map(|change| {
                Ok(CoachAcceptanceChangeBody {
                    acceptance_status: coach_acceptance_status_to_contract(change.status()),
                    reason: change.reason().map(|reason| reason.as_str().to_string()),
                    changed_by: account_id_uuid::format(change.changed_by()),
                    changed_at: date_time_to_contract(change.changed_at())?,
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}
