//! `/coaches/incomplete-profiles`。admin-web の `GET /coaches/incomplete-profiles`（`listIncompleteCoachProfiles`。本部が
//! 見る、プロフィールに足りない項目のあるコーチの一覧）。本部だけが呼べる。どの項目を足りないとみなすかは domain の
//! `MissingCoachProfileFields` の 1 か所で、項目の名前の文字列は [`crate::contract`] の対応表で決める。

use std::sync::Arc;

use application::{
    identity::{AccountRepository, TokenVerifier, account_id_uuid},
    profile::{CoachProfileRepository, IncompleteCoachProfile, ListIncompleteCoachProfiles},
};
use axum::{Json, Router, extract::State, routing::get};
use domain::identity::Role;
use serde::Serialize;

use crate::{
    auth::{TokenVerification, require_role},
    contract::{coach_profile_field_to_contract, date_time_to_contract},
    error::ApiError,
};

/// `/coaches/incomplete-profiles` のハンドラーが使うユースケース。
pub struct IncompleteCoachProfilesState<R, P> {
    pub list_incomplete_coach_profiles: ListIncompleteCoachProfiles<R, P>,
}

/// admin-web の `IncompleteCoachProfileList`。
#[derive(Serialize)]
pub struct IncompleteCoachProfileListBody {
    coaches: Vec<IncompleteCoachProfileBody>,
}

/// admin-web の `IncompleteCoachProfile`。最後に更新した時刻は記録がなければ null。
#[derive(Serialize)]
pub struct IncompleteCoachProfileBody {
    coach_id: String,
    display_name: String,
    missing_fields: Vec<&'static str>,
    updated_at: Option<String>,
    registered_at: String,
}

/// `GET /coaches/incomplete-profiles`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。
pub async fn list_incomplete_coach_profiles<R, P>(
    State(state): State<Arc<IncompleteCoachProfilesState<R, P>>>,
) -> Result<Json<IncompleteCoachProfileListBody>, ApiError>
where
    R: AccountRepository,
    P: CoachProfileRepository,
{
    let profiles = state
        .list_incomplete_coach_profiles
        .execute()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(IncompleteCoachProfileListBody {
        coaches: profiles
            .iter()
            .map(incomplete_coach_profile_body)
            .collect::<Result<_, _>>()?,
    }))
}

fn incomplete_coach_profile_body(
    profile: &IncompleteCoachProfile,
) -> Result<IncompleteCoachProfileBody, ApiError> {
    Ok(IncompleteCoachProfileBody {
        coach_id: account_id_uuid::format(profile.account.id()),
        display_name: profile.account.display_name().as_str().to_string(),
        missing_fields: profile
            .missing_fields
            .fields()
            .iter()
            .copied()
            .map(coach_profile_field_to_contract)
            .collect(),
        updated_at: profile.updated_at.map(date_time_to_contract).transpose()?,
        registered_at: date_time_to_contract(profile.account.registered_at())?,
    })
}

/// 本部が見る、プロフィールに足りない項目のあるコーチの一覧（admin-web の `GET /coaches/incomplete-profiles`）の
/// ルーター。組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// 本部だけ（コーチ・生徒は 403）。同じ接頭辞の `/coaches/{coach_id}` などは別のルーターで、パスの静的な部分が
/// 優先されるので `incomplete-profiles` はコーチの ID として読まれない。
pub fn incomplete_coach_profiles_router<R, P, T>(
    list_incomplete_coach_profiles: ListIncompleteCoachProfiles<R, P>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    P: CoachProfileRepository + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/coaches/incomplete-profiles",
            get(self::list_incomplete_coach_profiles::<R, P>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(IncompleteCoachProfilesState {
            list_incomplete_coach_profiles,
        }))
        .layer(TokenVerification::layer(tokens))
}
