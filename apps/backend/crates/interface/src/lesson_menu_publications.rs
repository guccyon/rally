//! `POST /lesson-menus/{lesson_menu_id}/unpublish`・`POST /lesson-menus/{lesson_menu_id}/republish`。admin-web の
//! `unpublishLessonMenu`・`republishLessonMenu`（本部がコーチに代わってレッスンメニューを非公開にする・公開に戻す）。
//! 本部だけが呼べる。非公開にした・公開に戻した記録の照会（`GET /lesson-menus/{lesson_menu_id}/publication-changes`）は
//! [`crate::lesson_menu_publication_changes`]。

use std::sync::Arc;

use application::{
    catalog::{
        ChangeLessonMenuPublicationError, LessonMenuRepository, ListLessonMenuPublicationChanges,
        RepublishLessonMenu, UnpublishLessonMenu, lesson_menu_id_uuid,
    },
    clock::Clock,
    identity::TokenVerifier,
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    routing::{get, post},
};
use domain::{
    catalog::{HeadquartersUnpublishReason, LessonMenuId},
    identity::Role,
};
use serde::Deserialize;

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    catalog_error::lesson_menu_error,
    error::ApiError,
    lesson_menu_publication_changes,
    lesson_menus::{LessonMenuBody, lesson_menu_body},
};

/// `/lesson-menus/{lesson_menu_id}/unpublish`・`/republish`・`/publication-changes` のハンドラーが使うユースケース。
pub struct LessonMenuPublicationsState<M, C> {
    pub unpublish_lesson_menu: UnpublishLessonMenu<M, C>,
    pub republish_lesson_menu: RepublishLessonMenu<M, C>,
    pub list_lesson_menu_publication_changes: ListLessonMenuPublicationChanges<M>,
}

/// admin-web の `UnpublishLessonMenuRequest`。本文ごと省いてよい。
#[derive(Deserialize, Default)]
pub struct UnpublishLessonMenuRequest {
    #[serde(default)]
    reason: Option<String>,
}

/// `POST /lesson-menus/{lesson_menu_id}/unpublish`。本部だけが呼べ（役割は `require_role` の層が確かめる）、
/// 非公開にしたメニューを返す。本文（任意の `reason`）は省いてよく、空の本文は理由なしとして扱う。非公開にした記録には、
/// トークンのアカウントを操作した本部として残す。
pub async fn unpublish_lesson_menu<M, C>(
    State(state): State<Arc<LessonMenuPublicationsState<M, C>>>,
    account: AuthenticatedAccount,
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
        .execute(lesson_menu_id, account.account_id(), reason)
        .await
        .map_err(publication_error)?;
    lesson_menu_body(&menu).map(Json)
}

/// `POST /lesson-menus/{lesson_menu_id}/republish`。本部だけが呼べ、公開中に戻したメニューを返す。リクエストの本文は読まない。
/// 公開に戻した記録には、トークンのアカウントを操作した本部として残す。
pub async fn republish_lesson_menu<M, C>(
    State(state): State<Arc<LessonMenuPublicationsState<M, C>>>,
    account: AuthenticatedAccount,
    Path(lesson_menu_id): Path<String>,
) -> Result<Json<LessonMenuBody>, ApiError>
where
    M: LessonMenuRepository,
    C: Clock,
{
    let lesson_menu_id = parse_lesson_menu_id(&lesson_menu_id)?;
    let menu = state
        .republish_lesson_menu
        .execute(lesson_menu_id, account.account_id())
        .await
        .map_err(publication_error)?;
    lesson_menu_body(&menu).map(Json)
}

/// 契約の 3 つの操作では、UUID でない ID はないメニューとして 404。
pub(crate) fn parse_lesson_menu_id(lesson_menu_id: &str) -> Result<LessonMenuId, ApiError> {
    lesson_menu_id_uuid::parse(lesson_menu_id).map_err(|_| lesson_menu_not_found())
}

pub(crate) fn lesson_menu_not_found() -> ApiError {
    ApiError::not_found("レッスンメニューがない")
}

fn publication_error(error: ChangeLessonMenuPublicationError) -> ApiError {
    match error {
        ChangeLessonMenuPublicationError::LessonMenuNotFound => lesson_menu_not_found(),
        ChangeLessonMenuPublicationError::Invalid(error) => lesson_menu_error(error),
        ChangeLessonMenuPublicationError::Unavailable(error) => ApiError::internal(error),
    }
}

/// 本部によるレッスンメニューの非公開・公開に戻す操作と、その記録の照会（admin-web の
/// `POST /lesson-menus/{lesson_menu_id}/unpublish`・`POST /lesson-menus/{lesson_menu_id}/republish`・
/// `GET /lesson-menus/{lesson_menu_id}/publication-changes`）のルーター。組み立て用のクレートは他の機能のルーターと `merge`
/// する（同じ検証器を渡す）。
///
/// どれも本部だけ（コーチ・生徒は 403）。
pub fn lesson_menu_publication_router<M, C, T>(
    unpublish_lesson_menu: UnpublishLessonMenu<M, C>,
    republish_lesson_menu: RepublishLessonMenu<M, C>,
    list_lesson_menu_publication_changes: ListLessonMenuPublicationChanges<M>,
    tokens: T,
) -> Router
where
    M: LessonMenuRepository + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/lesson-menus/{lesson_menu_id}/unpublish",
            post(self::unpublish_lesson_menu::<M, C>),
        )
        .route(
            "/lesson-menus/{lesson_menu_id}/republish",
            post(self::republish_lesson_menu::<M, C>),
        )
        .route(
            "/lesson-menus/{lesson_menu_id}/publication-changes",
            get(lesson_menu_publication_changes::list_lesson_menu_publication_changes::<M, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(LessonMenuPublicationsState {
            unpublish_lesson_menu,
            republish_lesson_menu,
            list_lesson_menu_publication_changes,
        }))
        .layer(TokenVerification::layer(tokens))
}
