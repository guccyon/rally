//! `/students/{student_id}/announcements`。admin-web の `GET /students/{student_id}/announcements`
//! （`listStudentAnnouncements`。本部が見る生徒 1 人の受け取ったお知らせと既読）。本部だけが呼べる。

use std::sync::Arc;

use application::{
    identity::{TokenVerifier, account_id_uuid},
    notification::{
        AnnouncementRecipientLookup, AnnouncementRepository, ListStudentAnnouncements,
        ListStudentAnnouncementsError, NotificationRepository, announcement_id_uuid,
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
    auth::{TokenVerification, require_role},
    contract::date_time_to_contract,
    error::ApiError,
};

/// `/students/{student_id}/announcements` のハンドラーが使うユースケース。
pub struct StudentAnnouncementsState<N, A, L> {
    pub list_student_announcements: ListStudentAnnouncements<N, A, L>,
}

/// ハンドラーが共有する [`StudentAnnouncementsState`]。
type Shared<N, A, L> = Arc<StudentAnnouncementsState<N, A, L>>;

/// admin-web の `StudentAnnouncement`。本文は載せない。
#[derive(Serialize)]
pub struct StudentAnnouncementBody {
    announcement_id: String,
    title: String,
    sent_at: String,
    is_read: bool,
    is_withdrawn: bool,
}

/// admin-web の `StudentAnnouncementList`。
#[derive(Serialize)]
pub struct StudentAnnouncementListBody {
    announcements: Vec<StudentAnnouncementBody>,
}

/// `GET /students/{student_id}/announcements`。本部だけが呼べる（役割は `require_role` の層が確かめる）。`student_id` が
/// UUID でない・生徒のアカウントがないときは 404。
pub async fn list_student_announcements<N, A, L>(
    State(state): State<Shared<N, A, L>>,
    Path(student_id): Path<String>,
) -> Result<Json<StudentAnnouncementListBody>, ApiError>
where
    N: NotificationRepository,
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
{
    let student_id =
        account_id_uuid::parse(&student_id).map_err(|_| ApiError::not_found("生徒がない"))?;
    let received = state
        .list_student_announcements
        .execute(student_id)
        .await
        .map_err(|error| match error {
            ListStudentAnnouncementsError::StudentNotFound => ApiError::not_found("生徒がない"),
            ListStudentAnnouncementsError::Unavailable(cause) => ApiError::internal(cause),
        })?;
    Ok(Json(StudentAnnouncementListBody {
        announcements: received
            .iter()
            .map(|received| {
                Ok(StudentAnnouncementBody {
                    announcement_id: announcement_id_uuid::format(received.announcement_id()),
                    title: received.title().as_str().to_string(),
                    sent_at: date_time_to_contract(received.sent_at())?,
                    is_read: received.is_read(),
                    is_withdrawn: received.is_withdrawn(),
                })
            })
            .collect::<Result<_, ApiError>>()?,
    }))
}

/// 本部が見る生徒 1 人の受け取ったお知らせ（admin-web の `GET /students/{student_id}/announcements`）のルーター。
/// 組み立て用のクレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /students/{student_id}/announcements` は本部だけ（コーチ・生徒は 403）。
pub fn student_announcements_router<N, A, L, T>(
    list_student_announcements: ListStudentAnnouncements<N, A, L>,
    tokens: T,
) -> Router
where
    N: NotificationRepository + 'static,
    A: AnnouncementRepository + 'static,
    L: AnnouncementRecipientLookup + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/students/{student_id}/announcements",
            get(self::list_student_announcements::<N, A, L>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(StudentAnnouncementsState {
            list_student_announcements,
        }))
        .layer(TokenVerification::layer(tokens))
}
