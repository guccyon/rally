//! `POST /lesson-menus/{lesson_menu_id}/unpublish`・`POST /lesson-menus/{lesson_menu_id}/republish`。admin-web の
//! `unpublishLessonMenu`・`republishLessonMenu`（本部がコーチに代わってレッスンメニューを非公開にする・公開に戻す）。
//! 本部だけが呼べる。

use std::sync::Arc;

use application::{
    catalog::{
        ChangeLessonMenuPublicationError, LessonMenuRepository, RepublishLessonMenu,
        UnpublishLessonMenu, lesson_menu_id_uuid,
    },
    clock::Clock,
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
};
use domain::catalog::{HeadquartersUnpublishReason, LessonMenuId};
use serde::Deserialize;

use crate::{
    catalog_error::lesson_menu_error,
    error::ApiError,
    lesson_menus::{LessonMenuBody, lesson_menu_body},
};

/// `/lesson-menus/{lesson_menu_id}/unpublish`・`/republish` のハンドラーが使うユースケース。
pub struct LessonMenuPublicationsState<M, C> {
    pub unpublish_lesson_menu: UnpublishLessonMenu<M, C>,
    pub republish_lesson_menu: RepublishLessonMenu<M>,
}

/// admin-web の `UnpublishLessonMenuRequest`。本文ごと省いてよい。
#[derive(Deserialize, Default)]
pub struct UnpublishLessonMenuRequest {
    #[serde(default)]
    reason: Option<String>,
}

/// `POST /lesson-menus/{lesson_menu_id}/unpublish`。本部だけが呼べ（役割は `require_role` の層が確かめる）、
/// 非公開にしたメニューを返す。本文（任意の `reason`）は省いてよく、空の本文は理由なしとして扱う。
pub async fn unpublish_lesson_menu<M, C>(
    State(state): State<Arc<LessonMenuPublicationsState<M, C>>>,
    Path(lesson_menu_id): Path<String>,
    body: Bytes,
) -> Result<Json<LessonMenuBody>, ApiError>
where
    M: LessonMenuRepository,
    C: Clock,
{
    let lesson_menu_id = parse_lesson_menu_id(&lesson_menu_id)?;
    let request: UnpublishLessonMenuRequest = if body.iter().all(u8::is_ascii_whitespace) {
        UnpublishLessonMenuRequest::default()
    } else {
        serde_json::from_slice(&body)
            .map_err(|error| ApiError::validation_failed(error.to_string()))?
    };
    let reason = HeadquartersUnpublishReason::new(request.reason.unwrap_or_default())?;
    let menu = state
        .unpublish_lesson_menu
        .execute(lesson_menu_id, reason)
        .await
        .map_err(publication_error)?;
    lesson_menu_body(&menu).map(Json)
}

/// `POST /lesson-menus/{lesson_menu_id}/republish`。本部だけが呼べ、公開中に戻したメニューを返す。リクエストの本文は読まない。
pub async fn republish_lesson_menu<M, C>(
    State(state): State<Arc<LessonMenuPublicationsState<M, C>>>,
    Path(lesson_menu_id): Path<String>,
) -> Result<Json<LessonMenuBody>, ApiError>
where
    M: LessonMenuRepository,
    C: Clock,
{
    let lesson_menu_id = parse_lesson_menu_id(&lesson_menu_id)?;
    let menu = state
        .republish_lesson_menu
        .execute(lesson_menu_id)
        .await
        .map_err(publication_error)?;
    lesson_menu_body(&menu).map(Json)
}

/// 契約の両操作では、UUID でない ID はないメニューとして 404。
fn parse_lesson_menu_id(lesson_menu_id: &str) -> Result<LessonMenuId, ApiError> {
    lesson_menu_id_uuid::parse(lesson_menu_id).map_err(|_| lesson_menu_not_found())
}

fn lesson_menu_not_found() -> ApiError {
    ApiError::not_found("レッスンメニューがない")
}

fn publication_error(error: ChangeLessonMenuPublicationError) -> ApiError {
    match error {
        ChangeLessonMenuPublicationError::LessonMenuNotFound => lesson_menu_not_found(),
        ChangeLessonMenuPublicationError::Invalid(error) => lesson_menu_error(error),
        ChangeLessonMenuPublicationError::Unavailable(error) => ApiError::internal(error),
    }
}
