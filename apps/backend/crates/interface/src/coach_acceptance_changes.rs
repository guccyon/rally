//! `GET /coaches/{coach_id}/acceptance-changes`。admin-web の `listCoachAcceptanceChanges`（本部がコーチの受付を止めた・
//! 再開した記録をたどる）。本部だけが呼べる。ルーターと状態は停止・再開（[`crate::coach_acceptances`]）と共有する。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::account_id_uuid,
    scheduling::{
        CoachAcceptanceRepository, CoachIntroductionLookup, ListCoachAcceptanceChangesError,
        ListCoachAcceptanceChangesQuery,
    },
};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};
use serde::{Deserialize, Serialize};

use crate::{
    coach_acceptances::{CoachAcceptancesState, parse_coach_id},
    contract::{coach_acceptance_status_to_contract, date_from_contract, date_time_to_contract},
    error::ApiError,
    scheduling_error::coach_not_found,
};

/// admin-web の `listCoachAcceptanceChanges` のクエリ。
#[derive(Deserialize)]
pub struct ListCoachAcceptanceChangesParams {
    from: Option<String>,
    to: Option<String>,
}

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
/// `from`・`to` を両方省けばすべて、どちらかがあれば変えた日付（日本時間）の期間で絞る。`from`・`to` が `YYYY-MM-DD` の
/// 日付でないときと、期間の違反は 422（期間の違反の応答は `listAnnouncements` と同じく [`crate::scheduling_error`] が
/// 決める）。UUID でない ID の 404 は日付より先に、期間の 422 はコーチがいるかより先に返す。
pub async fn list_coach_acceptance_changes<R, I, C>(
    State(state): State<Arc<CoachAcceptancesState<R, I, C>>>,
    Path(coach_id): Path<String>,
    params: Result<Query<ListCoachAcceptanceChangesParams>, QueryRejection>,
) -> Result<Json<CoachAcceptanceChangeListBody>, ApiError>
where
    R: CoachAcceptanceRepository,
    I: CoachIntroductionLookup,
    C: Clock,
{
    let coach_id = parse_coach_id(&coach_id)?;
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let query = ListCoachAcceptanceChangesQuery {
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
    let changes = state
        .list_coach_acceptance_changes
        .execute(coach_id, query)
        .await
        .map_err(|error| match error {
            ListCoachAcceptanceChangesError::Invalid(error) => ApiError::from(error),
            ListCoachAcceptanceChangesError::Unavailable(error) => ApiError::internal(error),
        })?
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
