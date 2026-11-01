//! `/me/profile` の生徒の分。student-app の `GET /me/profile`（`getMyStudentProfile`。自分の表示名などを見る）と
//! `PATCH /me/profile`（`updateMyStudentProfile`。自分の表示名を変える）。生徒だけが呼べる。同じパスの
//! `PUT /me/profile`（admin-web のコーチの自己紹介文）は [`crate::me`]。

use std::sync::Arc;

use application::{
    identity::{AccountRepository, GetAccount, TokenVerifier},
    profile::{UpdateStudentProfile, UpdateStudentProfileCommand, UpdateStudentProfileError},
};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    response::{IntoResponse, Response},
    routing::get,
};
use domain::identity::Role;
use serde::Deserialize;

use crate::{
    accounts::student_body,
    auth::{self, AuthenticatedAccount, TokenVerification, require_role},
    error::ApiError,
};

/// student-app の `/me/profile` のハンドラーが使うユースケース。
pub struct MyProfileState<R> {
    pub get_account: GetAccount<R>,
    pub update_student_profile: UpdateStudentProfile<R>,
}

/// student-app の `UpdateStudentProfileRequest`。値の検証は domain で行う。
#[derive(Deserialize)]
pub struct UpdateStudentProfileRequest {
    display_name: String,
}

/// `GET /me/profile`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンのアカウントで、
/// 存在しないか生徒でなければ admin-web の `GET /me` と同じく無効なトークンとして 401。
pub async fn get_my_student_profile<R>(
    State(state): State<Arc<MyProfileState<R>>>,
    account: AuthenticatedAccount,
) -> Response
where
    R: AccountRepository,
{
    match state.get_account.execute(account.account_id()).await {
        Ok(Some(found)) if found.role() == Role::Student => match student_body(&found) {
            Ok(body) => Json(body).into_response(),
            Err(error) => error.into_response(),
        },
        Ok(_) => auth::unauthorized("トークンが示す生徒のアカウントが存在しない"),
        Err(error) => ApiError::internal(error).into_response(),
    }
}

/// `PATCH /me/profile`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。書き換えるのはトークンのアカウントの
/// 表示名で、その生徒のアカウントがなければ `GET /me/profile` と同じく無効なトークンとして 401。
pub async fn update_my_student_profile<R>(
    State(state): State<Arc<MyProfileState<R>>>,
    account: AuthenticatedAccount,
    request: Result<Json<UpdateStudentProfileRequest>, JsonRejection>,
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
    let command = UpdateStudentProfileCommand {
        student_id: account.account_id(),
        display_name: request.display_name,
    };
    match state.update_student_profile.execute(command).await {
        Ok(updated) => match student_body(&updated) {
            Ok(body) => Json(body).into_response(),
            Err(error) => error.into_response(),
        },
        Err(UpdateStudentProfileError::Invalid(error)) => ApiError::from(error).into_response(),
        Err(UpdateStudentProfileError::StudentNotFound) => {
            auth::unauthorized("トークンが示す生徒のアカウントが存在しない")
        }
        Err(UpdateStudentProfileError::Unavailable(error)) => {
            ApiError::internal(error).into_response()
        }
    }
}

/// 生徒の自分のプロフィール（student-app の `GET /me/profile`・`PATCH /me/profile`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。同じパスの `PUT /me/profile`（admin-web
/// のコーチの自己紹介文）は [`crate::me_router`] にあり、`merge` で 1 つのパスにまとまる。
///
/// `GET`・`PATCH /me/profile` は生徒だけで、扱うのはトークンの生徒自身のアカウントだけ。
pub fn my_profile_router<R, T>(
    get_account: GetAccount<R>,
    update_student_profile: UpdateStudentProfile<R>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/me/profile",
            get(self::get_my_student_profile::<R>).patch(self::update_my_student_profile::<R>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(MyProfileState {
            get_account,
            update_student_profile,
        }))
        .layer(TokenVerification::layer(tokens))
}
