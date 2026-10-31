//! `/coaches/{coach_id}`。student-app の `GET /coaches/{coach_id}`（`getCoachProfile`。コーチの紹介）。生徒だけが呼べる。

use std::sync::Arc;

use application::{
    identity::{AccountRepository, account_id_uuid},
    profile::GetCoachProfile,
};
use axum::{
    Json,
    extract::{Path, State},
};
use domain::catalog::Sport;
use serde::Serialize;

use crate::{contract::sport_to_contract, error::ApiError};

/// `/coaches/{coach_id}` のハンドラーが使うユースケース。
pub struct CoachProfilesState<R> {
    pub get_coach_profile: GetCoachProfile<R>,
}

/// student-app の `CoachProfile`。自己紹介文は書いていなければ null。
#[derive(Serialize)]
pub struct CoachProfileBody {
    coach_id: String,
    display_name: String,
    bio: Option<String>,
    sports: Vec<&'static str>,
}

/// `GET /coaches/{coach_id}`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。ゴルフは golf-app の範囲
/// （student-app の `Sport` にない）なので載せず、ゴルフだけを提供するコーチは、コーチでない ID と同じく 404。
pub async fn get_coach_profile<R>(
    State(state): State<Arc<CoachProfilesState<R>>>,
    Path(coach_id): Path<String>,
) -> Result<Json<CoachProfileBody>, ApiError>
where
    R: AccountRepository,
{
    // 契約の getCoachProfile では、UUID でない ID はないものとして 404
    let coach_id =
        account_id_uuid::parse(&coach_id).map_err(|_| ApiError::not_found("コーチがない"))?;
    let profile = state
        .get_coach_profile
        .execute(coach_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("コーチがない"))?;
    let sports: Vec<&'static str> = profile
        .sports
        .iter()
        .filter(|sport| **sport != Sport::Golf)
        .map(|sport| sport_to_contract(*sport))
        .collect();
    if sports.is_empty() {
        return Err(ApiError::not_found("コーチがない"));
    }
    let bio = profile.bio.as_str();
    Ok(Json(CoachProfileBody {
        coach_id: account_id_uuid::format(coach_id),
        display_name: profile.display_name.as_str().to_string(),
        bio: (!bio.is_empty()).then(|| bio.to_string()),
        sports,
    }))
}
