//! `GET /lesson-menus/{lesson_menu_id}/publication-changes`。admin-web の `listLessonMenuPublicationChanges`（本部がレッスン
//! メニューを非公開にした・公開に戻した記録をたどる）。本部だけが呼べる。ルーターと状態は非公開・公開に戻す操作
//! （[`crate::lesson_menu_publications`]）と共有する。

use std::sync::Arc;

use application::{
    catalog::{LessonMenuRepository, lesson_menu_id_uuid},
    clock::Clock,
    identity::account_id_uuid,
};
use axum::{
    Json,
    extract::{Path, State},
};
use domain::catalog::{LessonMenuPublicationAction, LessonMenuPublicationChange};
use serde::Serialize;

use crate::{
    contract::{date_time_to_contract, lesson_menu_publication_action_to_contract},
    error::ApiError,
    lesson_menu_publications::{
        LessonMenuPublicationsState, lesson_menu_not_found, parse_lesson_menu_id,
    },
};

/// admin-web の `LessonMenuPublicationChangeList`。
#[derive(Serialize)]
pub struct LessonMenuPublicationChangeListBody {
    lesson_menu_id: String,
    changes: Vec<LessonMenuPublicationChangeBody>,
}

/// admin-web の `LessonMenuPublicationChange`。`reason` は非公開にした記録だけが持ち、公開に戻した記録は null。
#[derive(Serialize)]
struct LessonMenuPublicationChangeBody {
    action: &'static str,
    reason: Option<String>,
    changed_by: String,
    changed_at: String,
}

/// `GET /lesson-menus/{lesson_menu_id}/publication-changes`。本部だけが呼べ（役割は `require_role` の層が確かめる）、
/// 本部がメニューを非公開にした・公開に戻した記録を新しい順に返す。ない ID・UUID でない ID は 404。
pub async fn list_lesson_menu_publication_changes<M, C>(
    State(state): State<Arc<LessonMenuPublicationsState<M, C>>>,
    Path(lesson_menu_id): Path<String>,
) -> Result<Json<LessonMenuPublicationChangeListBody>, ApiError>
where
    M: LessonMenuRepository,
    C: Clock,
{
    let lesson_menu_id = parse_lesson_menu_id(&lesson_menu_id)?;
    let changes = state
        .list_lesson_menu_publication_changes
        .execute(lesson_menu_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(lesson_menu_not_found)?;
    Ok(Json(LessonMenuPublicationChangeListBody {
        lesson_menu_id: lesson_menu_id_uuid::format(lesson_menu_id),
        changes: changes
            .iter()
            .map(change_body)
            .collect::<Result<_, ApiError>>()?,
    }))
}

fn change_body(
    change: &LessonMenuPublicationChange,
) -> Result<LessonMenuPublicationChangeBody, ApiError> {
    let reason = match change.action() {
        LessonMenuPublicationAction::Unpublished { reason } => Some(reason.as_str().to_owned()),
        LessonMenuPublicationAction::Republished => None,
    };
    Ok(LessonMenuPublicationChangeBody {
        action: lesson_menu_publication_action_to_contract(change.action()),
        reason,
        changed_by: account_id_uuid::format(change.changed_by()),
        changed_at: date_time_to_contract(change.changed_at())?,
    })
}
