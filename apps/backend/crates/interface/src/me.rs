//! `/me`。admin-web の `GET /me`（`getMyAccount`。本人のアカウントを取る）と `PUT /me/profile`
//! （`updateMyCoachProfile`。コーチが自分の自己紹介文を書き換える）。`GET /me` は役割では絞らず、有効なトークンなら
//! 誰でも呼べる。`PUT /me/profile` はコーチだけ。

use std::sync::Arc;

use application::{
    identity::{AccountRepository, GetAccount, account_id_uuid},
    profile::{UpdateCoachProfile, UpdateCoachProfileCommand, UpdateCoachProfileError},
};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    response::{IntoResponse, Response},
};
use domain::identity::Account;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{self, AuthenticatedAccount},
    contract::role_to_contract,
    error::ApiError,
};

/// `/me` のハンドラーが使うユースケース。
pub struct MeState<R> {
    pub get_account: GetAccount<R>,
    pub update_coach_profile: UpdateCoachProfile<R>,
}

/// admin-web の `Account`。`bio` はコーチのときだけ載せる。
#[derive(Serialize)]
pub struct AccountBody {
    account_id: String,
    role: &'static str,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bio: Option<String>,
}

/// admin-web の `UpdateCoachProfileRequest`。
#[derive(Deserialize)]
pub struct UpdateCoachProfileRequest {
    bio: String,
}

/// `GET /me`。照会するのはトークンのアカウントで、存在しないか役割がトークンと合わなければ
/// 無効なトークンとして 401。
pub async fn get_my_account<R>(
    State(state): State<Arc<MeState<R>>>,
    account: AuthenticatedAccount,
) -> Response
where
    R: AccountRepository,
{
    match state.get_account.execute(account.account_id()).await {
        Ok(Some(found)) if found.role() == account.role() => {
            Json(account_body(&found)).into_response()
        }
        Ok(_) => auth::unauthorized("トークンが示すアカウントが存在しないか、役割が合わない"),
        Err(error) => ApiError::internal(error).into_response(),
    }
}

/// `PUT /me/profile`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。書き換えるのはトークンのアカウントの
/// 自己紹介文で、そのコーチのアカウントがなければ `GET /me` と同じく無効なトークンとして 401。
pub async fn update_my_coach_profile<R>(
    State(state): State<Arc<MeState<R>>>,
    account: AuthenticatedAccount,
    request: Result<Json<UpdateCoachProfileRequest>, JsonRejection>,
) -> Response
where
    R: AccountRepository,
{
    let Json(request) = match request {
        Ok(request) => request,
        Err(rejection) => {
            return ApiError::validation_failed(rejection.body_text()).into_response();
        }
    };
    let command = UpdateCoachProfileCommand {
        coach_id: account.account_id(),
        bio: request.bio,
    };
    match state.update_coach_profile.execute(command).await {
        Ok(updated) => Json(account_body(&updated)).into_response(),
        Err(UpdateCoachProfileError::Invalid(error)) => ApiError::from(error).into_response(),
        Err(UpdateCoachProfileError::CoachNotFound) => {
            auth::unauthorized("トークンが示すコーチのアカウントが存在しない")
        }
        Err(UpdateCoachProfileError::Unavailable(error)) => {
            ApiError::internal(error).into_response()
        }
    }
}

fn account_body(account: &Account) -> AccountBody {
    AccountBody {
        account_id: account_id_uuid::format(account.id()),
        role: role_to_contract(account.role()),
        display_name: account.display_name().as_str().to_string(),
        bio: account
            .as_coach()
            .map(|coach| coach.bio().as_str().to_string()),
    }
}
