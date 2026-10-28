//! `/notification-settings`。student-app の `GET /notification-settings/me`（`getMyNotificationSettings`）と
//! `PUT /notification-settings/me`（`updateMyNotificationSettings`）。生徒だけが呼べる。
//!
//! 通知の種類（domain の `NotificationKind`）と契約の項目名の対応はこのファイルだけに置く。

use std::sync::Arc;

use application::{
    identity::account_id_uuid,
    notification::{
        GetNotificationSettings, NotificationSettingsRepository, UpdateNotificationSettings,
        UpdateNotificationSettingsCommand,
    },
};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
};
use domain::notification::{NotificationKind, NotificationSettings};
use serde::{Deserialize, Serialize};

use crate::{auth::AuthenticatedAccount, error::ApiError};

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
}

/// student-app の `UpdateNotificationSettingsRequest`。書かなかった種類は `None`（今の設定のまま）。
#[derive(Deserialize)]
pub struct UpdateNotificationSettingsRequest {
    booking_confirmed: Option<bool>,
    video_review_returned: Option<bool>,
    message_received: Option<bool>,
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
    }
}
