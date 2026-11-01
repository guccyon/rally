//! `/me`。admin-web の `GET /me`（`getMyAccount`。本人のアカウントを取る）と `PUT /me/profile`
//! （`updateMyCoachProfile`。コーチが自分の自己紹介文を書き換える）。`GET /me` は役割では絞らず、有効なトークンなら
//! 誰でも呼べる。`PUT /me/profile` はコーチだけ。

use std::sync::Arc;

use application::{
    identity::{AccountRepository, GetAccount, TokenVerifier, account_id_uuid},
    profile::{
        CoachProfileRepository, GetCoachBio, UpdateCoachProfile, UpdateCoachProfileCommand,
        UpdateCoachProfileError,
    },
};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use domain::{
    identity::{Account, Role},
    profile::CoachBio,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{self, AuthenticatedAccount, TokenVerification, require_role},
    contract::role_to_contract,
    error::ApiError,
};

/// `/me` のハンドラーが使うユースケース。アカウントは identity、コーチの自己紹介文は profile のユースケースで扱う
/// （保存先は当面どちらも identity の表なので、同じ型 `R` を渡す）。
pub struct MeState<R> {
    pub get_account: GetAccount<R>,
    pub get_coach_bio: GetCoachBio<R>,
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
/// 無効なトークンとして 401。コーチなら自己紹介文を profile から読んで添える。
pub async fn get_my_account<R>(
    State(state): State<Arc<MeState<R>>>,
    account: AuthenticatedAccount,
) -> Response
where
    R: AccountRepository + CoachProfileRepository,
{
    let found = match state.get_account.execute(account.account_id()).await {
        Ok(Some(found)) if found.role() == account.role() => found,
        Ok(_) => {
            return auth::unauthorized("トークンが示すアカウントが存在しないか、役割が合わない");
        }
        Err(error) => return ApiError::internal(error).into_response(),
    };
    if found.as_coach().is_none() {
        return Json(account_body(&found, None)).into_response();
    }
    match state.get_coach_bio.execute(found.id()).await {
        Ok(Some(bio)) => Json(account_body(&found, Some(&bio))).into_response(),
        Ok(None) => auth::unauthorized("トークンが示すコーチのアカウントが存在しない"),
        Err(error) => ApiError::internal(error).into_response(),
    }
}

/// `PUT /me/profile`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。書き換えるのはトークンのアカウントの
/// 自己紹介文で、そのコーチのアカウントがなければ `GET /me` と同じく無効なトークンとして 401。応答は `GET /me` と同じ形で、
/// 自己紹介文を書き換えたあとにアカウントを identity から読んで組み立てる。
pub async fn update_my_coach_profile<R>(
    State(state): State<Arc<MeState<R>>>,
    account: AuthenticatedAccount,
    request: Result<Json<UpdateCoachProfileRequest>, JsonRejection>,
) -> Response
where
    R: AccountRepository + CoachProfileRepository,
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
    let bio = match state.update_coach_profile.execute(command).await {
        Ok(bio) => bio,
        Err(UpdateCoachProfileError::Invalid(error)) => {
            return ApiError::from(error).into_response();
        }
        Err(UpdateCoachProfileError::CoachNotFound) => {
            return auth::unauthorized("トークンが示すコーチのアカウントが存在しない");
        }
        Err(UpdateCoachProfileError::Unavailable(error)) => {
            return ApiError::internal(error).into_response();
        }
    };
    match state.get_account.execute(account.account_id()).await {
        Ok(Some(updated)) => Json(account_body(&updated, Some(&bio))).into_response(),
        Ok(None) => auth::unauthorized("トークンが示すコーチのアカウントが存在しない"),
        Err(error) => ApiError::internal(error).into_response(),
    }
}

/// `bio` はコーチのときだけ渡す（コーチでなければ `None`）。
fn account_body(account: &Account, bio: Option<&CoachBio>) -> AccountBody {
    AccountBody {
        account_id: account_id_uuid::format(account.id()),
        role: role_to_contract(account.role()),
        display_name: account.display_name().as_str().to_string(),
        bio: bio.map(|bio| bio.as_str().to_string()),
    }
}

/// ログイン中の本人（admin-web の `GET /me`・`PUT /me/profile`）のルーター。組み立て用のクレートは他の機能のルーターと
/// `merge` する（同じ検証器を渡す）。
///
/// `GET /me` は役割で絞らない。`PUT /me/profile`（自分の自己紹介文の変更）はコーチだけ。同じパスの student-app の
/// 生徒の分（`GET`・`PATCH /me/profile`）は [`crate::my_profile_router`] にあり、`merge` で 1 つのパスにまとまる。
pub fn me_router<R, T>(
    get_account: GetAccount<R>,
    get_coach_bio: GetCoachBio<R>,
    update_coach_profile: UpdateCoachProfile<R>,
    tokens: T,
) -> Router
where
    R: AccountRepository + CoachProfileRepository + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route("/me", get(get_my_account::<R>))
        .route(
            "/me/profile",
            put(update_my_coach_profile::<R>).route_layer(require_role(&[Role::Coach])),
        )
        .with_state(Arc::new(MeState {
            get_account,
            get_coach_bio,
            update_coach_profile,
        }))
        .layer(TokenVerification::layer(tokens))
}
