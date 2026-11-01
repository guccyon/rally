//! `/notification-settings`。student-app の `GET /notification-settings/me`（`getMyNotificationSettings`）と
//! `PUT /notification-settings/me`（`updateMyNotificationSettings`）。生徒だけが呼べる。
//!
//! 通知の種類（domain の `NotificationKind`）と契約の項目名の対応はこのファイルだけに置く。

use std::sync::Arc;

use application::{
    identity::{TokenVerifier, account_id_uuid},
    notification::{
        GetNotificationSettings, NotificationSettingsRepository, UpdateNotificationSettings,
        UpdateNotificationSettingsCommand,
    },
};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    routing::get,
};
use domain::{
    identity::Role,
    notification::{NotificationKind, NotificationSettings},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedAccount, TokenVerification, require_role},
    error::ApiError,
};

/// `/notification-settings` のハンドラーが使うユースケース。
pub struct NotificationSettingsState<S> {
    pub get_notification_settings: GetNotificationSettings<S>,
    pub update_notification_settings: UpdateNotificationSettings<S>,
}

/// student-app の `NotificationSettings`。
#[derive(Serialize)]
pub struct NotificationSettingsBody {
    student_id: String,
    booking_confirmed: bool,
    video_review_returned: bool,
    message_received: bool,
    announcement: bool,
}

/// student-app の `UpdateNotificationSettingsRequest`。書かなかった種類は `None`（今の設定のまま）。
#[derive(Deserialize)]
pub struct UpdateNotificationSettingsRequest {
    booking_confirmed: Option<bool>,
    video_review_returned: Option<bool>,
    message_received: Option<bool>,
    announcement: Option<bool>,
}

/// `GET /notification-settings/me`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンの
/// アカウント。
pub async fn get_my_notification_settings<S>(
    State(state): State<Arc<NotificationSettingsState<S>>>,
    account: AuthenticatedAccount,
) -> Result<Json<NotificationSettingsBody>, ApiError>
where
    S: NotificationSettingsRepository,
{
    let settings = state
        .get_notification_settings
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(notification_settings_body(&settings)))
}

/// `PUT /notification-settings/me`。生徒だけが呼べる（役割は `require_role` の層が確かめる）。変えるのはトークンの
/// アカウントの設定。
pub async fn update_my_notification_settings<S>(
    State(state): State<Arc<NotificationSettingsState<S>>>,
    account: AuthenticatedAccount,
    request: Result<Json<UpdateNotificationSettingsRequest>, JsonRejection>,
) -> Result<Json<NotificationSettingsBody>, ApiError>
where
    S: NotificationSettingsRepository,
{
    let Json(request) =
        request.map_err(|rejection| ApiError::validation_failed(rejection.body_text()))?;
    let changes = [
        (
            NotificationKind::BookingConfirmed,
            request.booking_confirmed,
        ),
        (
            NotificationKind::VideoReviewReturned,
            request.video_review_returned,
        ),
        (NotificationKind::MessageReceived, request.message_received),
        (NotificationKind::Announcement, request.announcement),
    ]
    .into_iter()
    .filter_map(|(kind, enabled)| enabled.map(|enabled| (kind, enabled)))
    .collect();
    let settings = state
        .update_notification_settings
        .execute(UpdateNotificationSettingsCommand {
            account_id: account.account_id(),
            changes,
        })
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(notification_settings_body(&settings)))
}

fn notification_settings_body(settings: &NotificationSettings) -> NotificationSettingsBody {
    NotificationSettingsBody {
        student_id: account_id_uuid::format(settings.account_id()),
        booking_confirmed: settings.is_enabled(NotificationKind::BookingConfirmed),
        video_review_returned: settings.is_enabled(NotificationKind::VideoReviewReturned),
        message_received: settings.is_enabled(NotificationKind::MessageReceived),
        announcement: settings.is_enabled(NotificationKind::Announcement),
    }
}

/// 自分の通知の受け取り設定（student-app の `GET`・`PUT /notification-settings/me`）のルーター。組み立て用のクレートは
/// 他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// どちらも生徒だけ。
pub fn notification_settings_router<S, T>(
    get_notification_settings: GetNotificationSettings<S>,
    update_notification_settings: UpdateNotificationSettings<S>,
    tokens: T,
) -> Router
where
    S: NotificationSettingsRepository + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/notification-settings/me",
            get(get_my_notification_settings::<S>).put(update_my_notification_settings::<S>),
        )
        .route_layer(require_role(&[Role::Student]))
        .with_state(Arc::new(NotificationSettingsState {
            get_notification_settings,
            update_notification_settings,
        }))
        .layer(TokenVerification::layer(tokens))
}
