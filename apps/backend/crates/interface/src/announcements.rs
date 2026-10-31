//! `/announcements`。admin-web の `POST /announcements`（`sendAnnouncement`。本部が生徒にお知らせを送る）と
//! `GET /announcements`（`listAnnouncements`。本部が送ったお知らせの一覧）。どちらも本部だけが呼べる。
//!
//! 受け取った生徒のお知らせ（`GET /notifications/me`）の本文は `notifications` が決める。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::account_id_uuid,
    notification::{
        AnnouncementRecipientLookup, AnnouncementRepository, ListAnnouncements,
        NotificationRepository, NotificationSettingsRepository, PushSender, SendAnnouncement,
        SendAnnouncementCommand, SendAnnouncementError, announcement_id_uuid,
    },
};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::notification::{Announcement, AnnouncementBody, AnnouncementTitle, NotificationError};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthenticatedAccount,
    contract::{admin_web_sport, date_time_to_contract, sport_to_contract},
    error::ApiError,
    message::notification_error,
};

/// `/announcements` のハンドラーが使うユースケース。
pub struct AnnouncementsState<A, L, N, S, P, C> {
    pub send_announcement: SendAnnouncement<A, L, N, S, P, C>,
    pub list_announcements: ListAnnouncements<A>,
}

/// ハンドラーが共有する [`AnnouncementsState`]。
type Shared<A, L, N, S, P, C> = Arc<AnnouncementsState<A, L, N, S, P, C>>;

/// admin-web の `SendAnnouncementRequest`。`sport` は省くか null なら生徒全員。
#[derive(Deserialize)]
pub struct SendAnnouncementRequest {
    title: String,
    body: String,
    sport: Option<String>,
}

/// admin-web の `Announcement`。
#[derive(Serialize)]
pub struct AnnouncementBodyJson {
    announcement_id: String,
    sender_id: String,
    title: String,
    body: String,
    sport: Option<&'static str>,
    recipient_count: u32,
    sent_at: String,
}

/// admin-web の `AnnouncementList`。
#[derive(Serialize)]
pub struct AnnouncementListBody {
    announcements: Vec<AnnouncementBodyJson>,
}

/// `POST /announcements`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。送ったのはトークンの
/// 本部のアカウントとして残す。対象の生徒がいなければ 404、題・本文・スポーツが不正なら 422 で、どちらも何も残さない。
pub async fn send_announcement<A, L, N, S, P, C>(
    State(state): State<Shared<A, L, N, S, P, C>>,
    account: AuthenticatedAccount,
    request: Result<Json<SendAnnouncementRequest>, JsonRejection>,
) -> Result<Response, ApiError>
where
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
    N: NotificationRepository,
    S: NotificationSettingsRepository,
    P: PushSender,
    C: Clock,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let invalid =
        |error: NotificationError| ApiError::validation_failed(notification_error(&error));
    let command = SendAnnouncementCommand {
        sender_id: account.account_id(),
        title: AnnouncementTitle::new(request.title).map_err(invalid)?,
        body: AnnouncementBody::new(request.body).map_err(invalid)?,
        sport: request.sport.as_deref().map(admin_web_sport).transpose()?,
    };
    match state.send_announcement.execute(command).await {
        Ok(announcement) => {
            Ok((StatusCode::CREATED, Json(announcement_body(&announcement)?)).into_response())
        }
        Err(SendAnnouncementError::Invalid(
            error @ NotificationError::AnnouncementWithoutRecipients,
        )) => Err(ApiError::not_found(notification_error(&error))),
        Err(SendAnnouncementError::Invalid(error)) => Err(invalid(error)),
        Err(SendAnnouncementError::Unavailable(error)) => Err(ApiError::internal(error)),
    }
}

/// `GET /announcements`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。送ったお知らせを
/// 送った時刻の新しい順で返す（どの本部のアカウントが送ったかで絞らない）。
pub async fn list_announcements<A, L, N, S, P, C>(
    State(state): State<Shared<A, L, N, S, P, C>>,
) -> Result<Json<AnnouncementListBody>, ApiError>
where
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
    N: NotificationRepository,
    S: NotificationSettingsRepository,
    P: PushSender,
    C: Clock,
{
    let announcements = state
        .list_announcements
        .execute()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(AnnouncementListBody {
        announcements: announcements
            .iter()
            .map(announcement_body)
            .collect::<Result<_, _>>()?,
    }))
}

fn announcement_body(announcement: &Announcement) -> Result<AnnouncementBodyJson, ApiError> {
    Ok(AnnouncementBodyJson {
        announcement_id: announcement_id_uuid::format(announcement.id()),
        sender_id: account_id_uuid::format(announcement.sender_id()),
        title: announcement.title().as_str().to_string(),
        body: announcement.body().as_str().to_string(),
        sport: announcement.sport().map(sport_to_contract),
        recipient_count: announcement.recipient_count(),
        sent_at: date_time_to_contract(announcement.sent_at())?,
    })
}
