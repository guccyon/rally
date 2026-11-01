//! `/announcements`。admin-web の `POST /announcements`（`sendAnnouncement`。本部が生徒にお知らせを送る）と
//! `GET /announcements`（`listAnnouncements`。本部が送ったお知らせの一覧）と
//! `POST /announcements/{announcement_id}/withdraw`（`withdrawAnnouncement`。本部が送ったお知らせを取り下げる）。
//! どれも本部だけが呼べる。
//!
//! 受け取った生徒のお知らせ（`GET /notifications/me`）の本文は `notifications` が決める。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{TokenVerifier, account_id_uuid},
    notification::{
        AnnouncementRecipientLookup, AnnouncementRepository, ListAnnouncements,
        NotificationRepository, NotificationSettingsRepository, PushSender, SendAnnouncement,
        SendAnnouncementCommand, SendAnnouncementError, WithdrawAnnouncement,
        WithdrawAnnouncementError, announcement_id_uuid,
    },
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use domain::{
    identity::Role,
    notification::{Announcement, AnnouncementBody, AnnouncementTitle, NotificationError},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    contract::{admin_web_sport, date_time_to_contract, sport_to_contract},
    error::ApiError,
    message::notification_error,
};

/// `/announcements` のハンドラーが使うユースケース。
pub struct AnnouncementsState<A, L, N, S, P, C> {
    pub send_announcement: SendAnnouncement<A, L, N, S, P, C>,
    pub list_announcements: ListAnnouncements<A>,
    pub withdraw_announcement: WithdrawAnnouncement<A, C>,
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
    withdrawal: Option<AnnouncementWithdrawalBody>,
}

/// admin-web の `AnnouncementWithdrawal`。
#[derive(Serialize)]
pub struct AnnouncementWithdrawalBody {
    withdrawn_by: String,
    withdrawn_at: String,
}

/// admin-web の `AnnouncementList`。
#[derive(Serialize)]
pub struct AnnouncementListBody {
    announcements: Vec<AnnouncementBodyJson>,
}

/// `POST /announcements/{announcement_id}/withdraw`（admin-web）。本部だけが呼べる（役割は `require_role` の層が確かめる）。
/// 取り下げたのはトークンの本部のアカウントとして残し、取り下げたあとの記録を返す。リクエストの本文は読まない。
/// ない・UUID でないお知らせは 404、取り下げ済みのお知らせは 409 `announcement_already_withdrawn` で、どちらも何も変えない。
pub async fn withdraw_announcement<A, L, N, S, P, C>(
    State(state): State<Shared<A, L, N, S, P, C>>,
    account: AuthenticatedAccount,
    Path(announcement_id): Path<String>,
) -> Result<Json<AnnouncementBodyJson>, ApiError>
where
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
    N: NotificationRepository,
    S: NotificationSettingsRepository,
    P: PushSender,
    C: Clock,
{
    let not_found = || ApiError::not_found("お知らせがない");
    let announcement_id = announcement_id_uuid::parse(&announcement_id).map_err(|_| not_found())?;
    match state
        .withdraw_announcement
        .execute(announcement_id, account.account_id())
        .await
    {
        Ok(announcement) => announcement_body(&announcement).map(Json),
        Err(WithdrawAnnouncementError::AnnouncementNotFound) => Err(not_found()),
        Err(WithdrawAnnouncementError::Invalid(
            error @ NotificationError::AnnouncementAlreadyWithdrawn,
        )) => Err(ApiError::conflict(
            "announcement_already_withdrawn",
            notification_error(&error),
        )),
        Err(WithdrawAnnouncementError::Invalid(error)) => Err(ApiError::internal(error)),
        Err(WithdrawAnnouncementError::Unavailable(error)) => Err(ApiError::internal(error)),
    }
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
        withdrawal: announcement
            .withdrawal()
            .map(|withdrawal| {
                Ok::<_, ApiError>(AnnouncementWithdrawalBody {
                    withdrawn_by: account_id_uuid::format(withdrawal.withdrawn_by()),
                    withdrawn_at: date_time_to_contract(withdrawal.withdrawn_at())?,
                })
            })
            .transpose()?,
    })
}

/// 本部からのお知らせ（admin-web の `POST /announcements`・`GET /announcements`・
/// `POST /announcements/{announcement_id}/withdraw`）のルーター。組み立て用のクレートは他の機能のルーターと `merge`
/// する（同じ検証器を渡す）。
///
/// どれも本部だけ（コーチ・生徒は 403）。受け取った生徒は自分のお知らせ（[`crate::notifications_router`]）で読む。
pub fn announcements_router<A, L, N, S, P, C, T>(
    send_announcement: SendAnnouncement<A, L, N, S, P, C>,
    list_announcements: ListAnnouncements<A>,
    withdraw_announcement: WithdrawAnnouncement<A, C>,
    tokens: T,
) -> Router
where
    A: AnnouncementRepository + 'static,
    L: AnnouncementRecipientLookup + 'static,
    N: NotificationRepository + 'static,
    S: NotificationSettingsRepository + 'static,
    P: PushSender + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/announcements",
            post(self::send_announcement::<A, L, N, S, P, C>)
                .get(self::list_announcements::<A, L, N, S, P, C>),
        )
        .route(
            "/announcements/{announcement_id}/withdraw",
            post(self::withdraw_announcement::<A, L, N, S, P, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(AnnouncementsState {
            send_announcement,
            list_announcements,
            withdraw_announcement,
        }))
        .layer(TokenVerification::layer(tokens))
}
