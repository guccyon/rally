//! コーチのレッスンメニュー（catalog）。admin-web の `GET /coaches/me/lesson-menus`（`listMyLessonMenus`）・
//! `POST /coaches/me/lesson-menus`（`createMyLessonMenu`）・`PUT /coaches/me/lesson-menus/{lesson_menu_id}`
//! （`updateMyLessonMenu`）はコーチだけで、扱うのはトークンのコーチ自身のメニューだけ。student-app の
//! `GET /coaches/{coach_id}/lesson-menus`（`listCoachLessonMenus`）は生徒だけで、公開中のメニューだけを返す
//! （ゴルフは golf-app の範囲なので載せない）。他のコーチのメニューは、あることも知らせない（404）。

use std::sync::Arc;

use application::{
    catalog::{
        CreateLessonMenu, CreateLessonMenuCommand, CreateLessonMenuError, LessonMenuRepository,
        ListCoachLessonMenus, ListMyLessonMenus, UpdateLessonMenu, UpdateLessonMenuCommand,
        UpdateLessonMenuError, lesson_menu_id_uuid,
    },
    clock::Clock,
    identity::{AccountRepository, account_id_uuid},
};
use axum::{
    Json,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{
    DomainError,
    catalog::{
        LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
        LessonMenuName,
    },
    identity::{AccountId, Sport},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{self, AuthenticatedAccount},
    contract::{
        admin_web_sport, date_time_to_contract, lesson_type_from_contract, lesson_type_to_contract,
        sport_to_contract, student_app_sport,
    },
    error::ApiError,
};

/// レッスンメニューのハンドラーが使うユースケース。
pub struct LessonMenusState<M, R, C> {
    pub create_lesson_menu: CreateLessonMenu<M, R, C>,
    pub update_lesson_menu: UpdateLessonMenu<M, R>,
    pub list_my_lesson_menus: ListMyLessonMenus<M>,
    pub list_coach_lesson_menus: ListCoachLessonMenus<M>,
}

/// ハンドラーが共有する [`LessonMenusState`]。
type Shared<M, R, C> = Arc<LessonMenusState<M, R, C>>;

/// admin-web の `CreateLessonMenuRequest`。
#[derive(Deserialize)]
pub struct CreateLessonMenuRequest {
    sport: String,
    lesson_type: String,
    name: String,
    #[serde(default)]
    description: String,
    duration_minutes: Option<i64>,
    is_published: Option<bool>,
}

/// admin-web の `UpdateLessonMenuRequest`。
#[derive(Deserialize)]
pub struct UpdateLessonMenuRequest {
    sport: String,
    lesson_type: String,
    name: String,
    description: String,
    duration_minutes: Option<i64>,
    is_published: bool,
}

/// student-app の `listCoachLessonMenus` のクエリ。
#[derive(Deserialize)]
pub struct ListCoachLessonMenusParams {
    sport: Option<String>,
}

/// admin-web・student-app の `LessonMenu`。
#[derive(Serialize)]
pub struct LessonMenuBody {
    lesson_menu_id: String,
    coach_id: String,
    sport: &'static str,
    lesson_type: &'static str,
    name: String,
    description: String,
    duration_minutes: Option<u8>,
    is_published: bool,
    created_at: String,
}

/// admin-web・student-app の `LessonMenuList`。
#[derive(Serialize)]
pub struct LessonMenuListBody {
    coach_id: String,
    lesson_menus: Vec<LessonMenuBody>,
}

/// `GET /coaches/me/lesson-menus`（admin-web）。コーチだけが呼べる（役割は `require_role` の層が確かめる）。
/// 一覧にするのはトークンのコーチのメニューで、公開中かを問わない。
pub async fn list_my_lesson_menus<M, R, C>(
    State(state): State<Shared<M, R, C>>,
    account: AuthenticatedAccount,
) -> Result<Json<LessonMenuListBody>, ApiError>
where
    M: LessonMenuRepository,
    R: AccountRepository,
    C: Clock,
{
    let menus = state
        .list_my_lesson_menus
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    lesson_menu_list_body(account.account_id(), &menus).map(Json)
}

/// `POST /coaches/me/lesson-menus`（admin-web）。コーチだけが呼べる（役割は `require_role` の層が確かめる）。
/// 作るのはトークンのコーチのメニューで、そのコーチのアカウントがなければ `PUT /me/profile` と同じく無効なトークンとして 401。
pub async fn create_my_lesson_menu<M, R, C>(
    State(state): State<Shared<M, R, C>>,
    account: AuthenticatedAccount,
    request: Result<Json<CreateLessonMenuRequest>, JsonRejection>,
) -> Result<Response, ApiError>
where
    M: LessonMenuRepository,
    R: AccountRepository,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let content = lesson_menu_content(
        &request.sport,
        &request.lesson_type,
        request.name,
        request.description,
        request.duration_minutes,
    )?;
    let command = CreateLessonMenuCommand {
        coach_id: account.account_id(),
        content,
        is_published: request.is_published.unwrap_or(true),
    };
    match state.create_lesson_menu.execute(command).await {
        Ok(menu) => Ok((StatusCode::CREATED, Json(lesson_menu_body(&menu)?)).into_response()),
        Err(CreateLessonMenuError::Invalid(error)) => Err(ApiError::from(error)),
        Err(CreateLessonMenuError::CoachNotFound) => Ok(auth::unauthorized(
            "トークンが示すコーチのアカウントが存在しない",
        )),
        Err(CreateLessonMenuError::Unavailable(error)) => Err(ApiError::internal(error)),
    }
}

/// `PUT /coaches/me/lesson-menus/{lesson_menu_id}`（admin-web）。コーチだけが呼べる（役割は `require_role` の層が
/// 確かめる）。直せるのはトークンのコーチのメニューだけで、他のコーチのメニューはないものとして 404。
pub async fn update_my_lesson_menu<M, R, C>(
    State(state): State<Shared<M, R, C>>,
    account: AuthenticatedAccount,
    Path(lesson_menu_id): Path<String>,
    request: Result<Json<UpdateLessonMenuRequest>, JsonRejection>,
) -> Result<Response, ApiError>
where
    M: LessonMenuRepository,
    R: AccountRepository,
    C: Clock,
{
    // 契約の updateMyLessonMenu では、UUID でない ID はないものとして 404
    let lesson_menu_id = lesson_menu_id_uuid::parse(&lesson_menu_id)
        .map_err(|_| ApiError::not_found("レッスンメニューがない"))?;
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let content = lesson_menu_content(
        &request.sport,
        &request.lesson_type,
        request.name,
        request.description,
        request.duration_minutes,
    )?;
    let command = UpdateLessonMenuCommand {
        coach_id: account.account_id(),
        lesson_menu_id,
        content,
        is_published: request.is_published,
    };
    match state.update_lesson_menu.execute(command).await {
        Ok(menu) => Ok(Json(lesson_menu_body(&menu)?).into_response()),
        // 他のコーチのメニューは、あることも知らせない
        Err(
            UpdateLessonMenuError::LessonMenuNotFound
            | UpdateLessonMenuError::Invalid(DomainError::NotTheLessonMenuCoach),
        ) => Err(ApiError::not_found("レッスンメニューがない")),
        Err(UpdateLessonMenuError::Invalid(error)) => Err(ApiError::from(error)),
        Err(UpdateLessonMenuError::CoachNotFound) => Ok(auth::unauthorized(
            "トークンが示すコーチのアカウントが存在しない",
        )),
        Err(UpdateLessonMenuError::Unavailable(error)) => Err(ApiError::internal(error)),
    }
}

/// `GET /coaches/{coach_id}/lesson-menus`（student-app）。生徒だけが呼べる（役割は `require_role` の層が確かめる）。
/// 公開中のメニューだけを、`sport` があればそのスポーツに絞って返す。ゴルフのメニューは載せない（student-app の
/// `Sport` にない）。
pub async fn list_coach_lesson_menus<M, R, C>(
    State(state): State<Shared<M, R, C>>,
    Path(coach_id): Path<String>,
    params: Result<Query<ListCoachLessonMenusParams>, QueryRejection>,
) -> Result<Json<LessonMenuListBody>, ApiError>
where
    M: LessonMenuRepository,
    R: AccountRepository,
    C: Clock,
{
    // 契約の listCoachLessonMenus では、UUID でない ID はないものとして 404
    let coach_id =
        account_id_uuid::parse(&coach_id).map_err(|_| ApiError::not_found("コーチがない"))?;
    let Query(params) =
        params.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let sport = params.sport.as_deref().map(student_app_sport).transpose()?;

    let menus: Vec<LessonMenu> = state
        .list_coach_lesson_menus
        .execute(coach_id, sport)
        .await
        .map_err(ApiError::internal)?
        .into_iter()
        .filter(|menu| menu.sport() != Sport::Golf)
        .collect();
    lesson_menu_list_body(coach_id, &menus).map(Json)
}

/// リクエストの項目からメニューの中身を組み立てる。契約の値でないもの・不変条件に合わないものは 422。
fn lesson_menu_content(
    sport: &str,
    lesson_type: &str,
    name: String,
    description: String,
    duration_minutes: Option<i64>,
) -> Result<LessonMenuContent, ApiError> {
    let duration = duration_minutes
        .map(LessonDuration::from_minutes)
        .transpose()?;
    Ok(LessonMenuContent {
        sport: admin_web_sport(sport)?,
        format: LessonFormat::new(lesson_type_from_contract(lesson_type)?, duration)?,
        name: LessonMenuName::new(name)?,
        description: LessonMenuDescription::new(description)?,
    })
}

fn lesson_menu_body(menu: &LessonMenu) -> Result<LessonMenuBody, ApiError> {
    Ok(LessonMenuBody {
        lesson_menu_id: lesson_menu_id_uuid::format(menu.id()),
        coach_id: account_id_uuid::format(menu.coach_id()),
        sport: sport_to_contract(menu.sport()),
        lesson_type: lesson_type_to_contract(menu.format().lesson_type()),
        name: menu.name().as_str().to_string(),
        description: menu.description().as_str().to_string(),
        duration_minutes: menu.format().duration().map(LessonDuration::minutes),
        is_published: menu.is_published(),
        created_at: date_time_to_contract(menu.created_at())?,
    })
}

fn lesson_menu_list_body(
    coach_id: AccountId,
    menus: &[LessonMenu],
) -> Result<LessonMenuListBody, ApiError> {
    Ok(LessonMenuListBody {
        coach_id: account_id_uuid::format(coach_id),
        lesson_menus: menus
            .iter()
            .map(lesson_menu_body)
            .collect::<Result<_, _>>()?,
    })
}
