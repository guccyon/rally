//! `/coaches`。admin-web の `GET /coaches`（`listCoaches`）。本部だけが呼べる。

use std::sync::Arc;

use application::identity::{AccountRepository, ListCoaches, account_id_uuid};
use axum::{Json, extract::State};
use domain::identity::Account;
use serde::Serialize;

use crate::{auth::AuthenticatedAccount, error::ApiError};

/// `/coaches` のハンドラーが使うユースケース。
pub struct CoachesState<R> {
    pub list_coaches: ListCoaches<R>,
}

/// admin-web の `CoachList`。
#[derive(Serialize)]
pub struct CoachListBody {
    coaches: Vec<CoachBody>,
}

/// admin-web の `Coach`。
#[derive(Serialize)]
pub struct CoachBody {
    coach_id: String,
    display_name: String,
}

/// `GET /coaches`。本部だけが呼べる（役割は `require_role` の層が確かめる）。
pub async fn list_coaches<R>(
    State(state): State<Arc<CoachesState<R>>>,
    _account: AuthenticatedAccount,
) -> Result<Json<CoachListBody>, ApiError>
where
    R: AccountRepository,
{
    let coaches = state
        .list_coaches
        .execute()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(CoachListBody {
        coaches: coaches.iter().map(coach_body).collect(),
    }))
}

fn coach_body(account: &Account) -> CoachBody {
    CoachBody {
        coach_id: account_id_uuid::format(account.id()),
        display_name: account.display_name().as_str().to_string(),
    }
}
