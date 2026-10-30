//! コーチのレッスンメニュー（catalog）。admin-web の `GET /coaches/me/lesson-menus`（`listMyLessonMenus`）・
//! `POST /coaches/me/lesson-menus`（`createLessonMenu`）・`PUT /coaches/me/lesson-menus/{lesson_menu_id}`
//! （`updateLessonMenu`）はコーチだけで、扱うのはトークンのコーチ本人のメニュー。student-app の
//! `GET /coaches/{coach_id}/lesson-menus`（`listCoachLessonMenus`）は生徒だけで、公開中のメニューだけを返す。
//! 別のコーチのメニューには、メニューがあることも知らせない（404）。

use std::sync::Arc;

use application::{
    catalog::{
        CoachLookup, CreateLessonMenu, CreateLessonMenuCommand, CreateLessonMenuError,
        LessonMenuRepository, ListCoachLessonMenus, ListMyLessonMenus, UpdateLessonMenu,
        UpdateLessonMenuCommand, UpdateLessonMenuError, lesson_menu_id_uuid,
    },
    clock::Clock,
    identity::account_id_uuid,
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
        LessonDuration, LessonMenu, LessonMenuContent, LessonMenuDescription, LessonMenuName,
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
pub struct LessonMenusState<M, K, C> {
    pub create_lesson_menu: CreateLessonMenu<M, K, C>,
    pub update_lesson_menu: UpdateLessonMenu<M, C>,
    pub list_my_lesson_menus: ListMyLessonMenus<M>,
    pub list_coach_lesson_menus: ListCoachLessonMenus<M>,
}

/// ハンドラーが共有する [`LessonMenusState`]。
type Shared<M, K, C> = Arc<LessonMenusState<M, K, C>>;

/// admin-web の `CreateLessonMenuRequest`。`description`・`duration_minutes`・`is_published` は任意。
#[derive(Deserialize)]
pub struct CreateLessonMenuRequest {
    sport: String,
    lesson_type: String,
    name: String,
    description: Option<String>,
    duration_minutes: Option<i64>,
    is_published: Option<bool>,
}

/// admin-web の `UpdateLessonMenuRequest`。`duration_minutes` は任意（非同期レッスンでは省く）。
#[derive(Deserialize)]
pub struct UpdateLessonMenuRequest {
    name: String,
    description: String,
    duration_minutes: Option<i64>,
    is_published: bool,
}

/// student-app の `listCoachLessonMenus` のクエリ。値の検証はハンドラーで行う。
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
    updated_at: String,
}

/// admin-web・student-app の `LessonMenuList`。
#[derive(Serialize)]
pub struct LessonMenuListBody {
    coach_id: String,
    lesson_menus: Vec<LessonMenuBody>,
}

/// `GET /coaches/me/lesson-menus`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。読むのはトークンの
/// コーチのメニューで、非公開も含む。
pub async fn list_my_lesson_menus<M, K, C>(
    State(state): State<Shared<M, K, C>>,
    account: AuthenticatedAccount,
) -> Result<Json<LessonMenuListBody>, ApiError>
where
    M: LessonMenuRepository,
    K: CoachLookup,
    C: Clock,
{
    let coach_id = account.account_id();
    let menus = state
        .list_my_lesson_menus
        .execute(coach_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(lesson_menu_list_body(coach_id, &menus)?))
}

/// `POST /coaches/me/lesson-menus`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。作るのはトークンの
/// コーチのメニューで、そのコーチのアカウントがなければ `PUT /me/profile` と同じく無効なトークンとして 401。
pub async fn create_lesson_menu<M, K, C>(
    State(state): State<Shared<M, K, C>>,
    account: AuthenticatedAccount,
    request: Result<Json<CreateLessonMenuRequest>, JsonRejection>,
) -> Response
where
    M: LessonMenuRepository,
    K: CoachLookup,
    C: Clock,
{
    let command = match create_command(account.account_id(), request) {
        Ok(command) => command,
        Err(error) => return error.into_response(),
    };
    match state.create_lesson_menu.execute(command).await {
        Ok(menu) => match lesson_menu_body(&menu) {
            Ok(body) => (StatusCode::CREATED, Json(body)).into_response(),
            Err(error) => error.into_response(),
        },
        Err(CreateLessonMenuError::CoachNotFound) => {
            auth::unauthorized("トークンが示すコーチのアカウントが存在しない")
        }
        Err(CreateLessonMenuError::Invalid(error)) => ApiError::from(error).into_response(),
        Err(CreateLessonMenuError::Unavailable(error)) => ApiError::internal(error).into_response(),
    }
}

/// `PUT /coaches/me/lesson-menus/{lesson_menu_id}`。コーチだけが呼べる（役割は `require_role` の層が確かめる）。
/// 直せるのはトークンのコーチのメニューだけで、別のコーチのメニューはないものとして 404。
pub async fn update_lesson_menu<M, K, C>(
    State(state): State<Shared<M, K, C>>,
    account: AuthenticatedAccount,
    Path(lesson_menu_id): Path<String>,
    request: Result<Json<UpdateLessonMenuRequest>, JsonRejection>,
) -> Result<Json<LessonMenuBody>, ApiError>
where
    M: LessonMenuRepository,
    K: CoachLookup,
    C: Clock,
{
    // 契約の updateLessonMenu は、UUID でない ID をないものとして 404 にする
    let lesson_menu_id = lesson_menu_id_uuid::parse(&lesson_menu_id)
        .map_err(|_| ApiError::not_found("レッスンメニューがない"))?;
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let command = UpdateLessonMenuCommand {
        account_id: account.account_id(),
        lesson_menu_id,
        content: LessonMenuContent {
            name: LessonMenuName::new(request.name)?,
            description: LessonMenuDescription::new(request.description)?,
            duration: request
                .duration_minutes
                .map(LessonDuration::new)
                .transpose()?,
            is_published: request.is_published,
        },
    };

    let menu = state
        .update_lesson_menu
        .execute(command)
        .await
        .map_err(|error| match error {
            // 別のコーチのメニューは、あることも知らせない
            UpdateLessonMenuError::LessonMenuNotFound
            | UpdateLessonMenuError::Invalid(DomainError::NotTheLessonMenuCoach) => {
                ApiError::not_found("レッスンメニューがない")
            }
            UpdateLessonMenuError::Invalid(error) => ApiError::from(error),
            UpdateLessonMenuError::Unavailable(error) => ApiError::internal(error),
        })?;
    Ok(Json(lesson_menu_body(&menu)?))
}

/// `GET /coaches/{coach_id}/lesson-menus`（student-app）。生徒だけが呼べる（役割は `require_role` の層が確かめる）。
/// 公開中のメニューだけを返し、student-app はゴルフを扱わないのでゴルフのメニューは返さない（`sport=golf` は 422）。
pub async fn list_coach_lesson_menus<M, K, C>(
    State(state): State<Shared<M, K, C>>,
    _account: AuthenticatedAccount,
    Path(coach_id): Path<String>,
    params: Result<Query<ListCoachLessonMenusParams>, QueryRejection>,
) -> Result<Json<LessonMenuListBody>, ApiError>
where
    M: LessonMenuRepository,
    K: CoachLookup,
    C: Clock,
{
    // 契約の listCoachLessonMenus は、UUID でない ID をないものとして 404 にする
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
    Ok(Json(lesson_menu_list_body(coach_id, &menus)?))
}

fn create_command(
    coach_id: AccountId,
    request: Result<Json<CreateLessonMenuRequest>, JsonRejection>,
) -> Result<CreateLessonMenuCommand, ApiError> {
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    Ok(CreateLessonMenuCommand {
        coach_id,
        sport: admin_web_sport(&request.sport)?,
        lesson_type: lesson_type_from_contract(&request.lesson_type)?,
        content: LessonMenuContent {
            name: LessonMenuName::new(request.name)?,
            description: request
                .description
                .map(LessonMenuDescription::new)
                .transpose()?
                .unwrap_or_default(),
            duration: request
                .duration_minutes
                .map(LessonDuration::new)
                .transpose()?,
            is_published: request.is_published.unwrap_or(true),
        },
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

fn lesson_menu_body(menu: &LessonMenu) -> Result<LessonMenuBody, ApiError> {
    Ok(LessonMenuBody {
        lesson_menu_id: lesson_menu_id_uuid::format(menu.id()),
        coach_id: account_id_uuid::format(menu.coach_id()),
        sport: sport_to_contract(menu.sport()),
        lesson_type: lesson_type_to_contract(menu.lesson_type()),
        name: menu.name().as_str().to_string(),
        description: menu.description().as_str().to_string(),
        duration_minutes: menu.duration().map(LessonDuration::minutes),
        is_published: menu.is_published(),
        created_at: date_time_to_contract(menu.created_at())?,
        updated_at: date_time_to_contract(menu.updated_at())?,
    })
}
