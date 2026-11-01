//! `/coaches/{coach_id}`。student-app の `GET /coaches/{coach_id}`（`getCoachProfile`。コーチの紹介）。生徒だけが呼べる。

use std::sync::Arc;

use application::{
    identity::{AccountRepository, TokenVerifier, account_id_uuid},
    profile::{
        CoachFavoriteLookup, CoachProfileRepository, GetCoachProfile, GetCoachProfileCommand,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use domain::identity::Role;
use serde::Serialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::student_app_sports_to_contract,
    error::ApiError,
};

/// `/coaches/{coach_id}` のハンドラーが使うユースケース。
pub struct CoachProfilesState<R, F> {
    pub get_coach_profile: GetCoachProfile<R, F>,
}

/// student-app の `CoachProfile`。自己紹介文は書いていなければ null。
#[derive(Serialize)]
pub struct CoachProfileBody {
    coach_id: String,
    display_name: String,
    bio: Option<String>,
    sports: Vec<&'static str>,
    is_favorite: bool,
}

/// `GET /coaches/{coach_id}`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。ゴルフは golf-app の範囲
/// （student-app の `Sport` にない）なので載せず、ゴルフだけを提供するコーチは、コーチでない ID と同じく 404。
/// お気に入りかはトークンの生徒のお気に入りで答える。
pub async fn get_coach_profile<R, F>(
    State(state): State<Arc<CoachProfilesState<R, F>>>,
    account: AuthenticatedAccount,
    Path(coach_id): Path<String>,
) -> Result<Json<CoachProfileBody>, ApiError>
where
    R: AccountRepository + CoachProfileRepository,
    F: CoachFavoriteLookup,
{
    // 契約の getCoachProfile では、UUID でない ID はないものとして 404
    let coach_id =
        account_id_uuid::parse(&coach_id).map_err(|_| ApiError::not_found("コーチがない"))?;
    let profile = state
        .get_coach_profile
        .execute(GetCoachProfileCommand {
            student_id: account.account_id(),
            coach_id,
        })
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("コーチがない"))?;
    let sports = student_app_sports_to_contract(&profile.sports);
    if sports.is_empty() {
        return Err(ApiError::not_found("コーチがない"));
    }
    let bio = profile.bio.as_str();
    Ok(Json(CoachProfileBody {
        coach_id: account_id_uuid::format(coach_id),
        display_name: profile.display_name.as_str().to_string(),
        bio: (!bio.is_empty()).then(|| bio.to_string()),
        sports,
        is_favorite: profile.is_favorite,
    }))
}

/// コーチの紹介（student-app の `GET /coaches/{coach_id}`）のルーター。組み立て用のクレートは他の機能のルーターと
/// `merge` する（同じ検証器を渡す）。
///
/// `GET /coaches/{coach_id}` は生徒だけ。
pub fn coach_profile_router<R, F, T>(get_coach_profile: GetCoachProfile<R, F>, tokens: T) -> Router
where
    R: AccountRepository + CoachProfileRepository + 'static,
    F: CoachFavoriteLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/coaches/{coach_id}", get(self::get_coach_profile::<R, F>))
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(CoachProfilesState { get_coach_profile }))
        .layer(TokenVerification::layer(tokens))
}
