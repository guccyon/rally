//! `/notifications/me`。student-app と admin-web の `GET /notifications/me`（`listMyNotifications`。自分のお知らせの一覧）と
//! `POST /notifications/me/read`（`markMyNotificationsRead`。すべて既読）。生徒とコーチが呼べ（本部は呼べない）、
//! どちらの契約も同じ形で、扱うのはトークンのアカウントが受け取るお知らせだけ。
//!
//! お知らせの種類（domain の `NotificationKind`）と契約の `NotificationKind` の値の対応と、プッシュ通知の中身から
//! 本文（利用者に見せる文言）を決める対応はこのファイルだけに置く。

use std::sync::Arc;

use application::{
    clock::Clock,
    notification::{
        ListMyNotifications, MarkMyNotificationsRead, NotificationRepository, notification_id_uuid,
    },
};
use axum::{Json, extract::State};
use domain::{
    messaging::MessageSender,
    notification::{Notification, NotificationKind, PushNotification},
};
use serde::Serialize;

use crate::{auth::AuthenticatedAccount, contract::date_time_to_contract, error::ApiError};

/// `/notifications/me` のハンドラーが使うユースケース。
pub struct NotificationsState<N, C> {
    pub list_my_notifications: ListMyNotifications<N>,
    pub mark_my_notifications_read: MarkMyNotificationsRead<N, C>,
}

/// student-app・admin-web の `NotificationList`。
#[derive(Serialize)]
pub struct NotificationListBody {
    notifications: Vec<NotificationBody>,
}

/// student-app・admin-web の `Notification`。
#[derive(Serialize)]
pub struct NotificationBody {
    notification_id: String,
    kind: &'static str,
    body: &'static str,
    created_at: String,
    is_read: bool,
}

/// `GET /notifications/me`。生徒とコーチが呼べる（役割は `require_role` の層が確かめる）。照会するのはトークンの
/// アカウントが受け取るお知らせ。
pub async fn list_my_notifications<N, C>(
    State(state): State<Arc<NotificationsState<N, C>>>,
    account: AuthenticatedAccount,
) -> Result<Json<NotificationListBody>, ApiError>
where
    N: NotificationRepository,
    C: Clock,
{
    let notifications = state
        .list_my_notifications
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(notification_list_body(&notifications)?))
}

/// `POST /notifications/me/read`。生徒とコーチが呼べる（役割は `require_role` の層が確かめる）。既読にするのはトークンの
/// アカウントが受け取るお知らせだけで、既読にしたあとの一覧を返す。
pub async fn mark_my_notifications_read<N, C>(
    State(state): State<Arc<NotificationsState<N, C>>>,
    account: AuthenticatedAccount,
) -> Result<Json<NotificationListBody>, ApiError>
where
    N: NotificationRepository,
    C: Clock,
{
    let notifications = state
        .mark_my_notifications_read
        .execute(account.account_id())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(notification_list_body(&notifications)?))
}

fn notification_list_body(
    notifications: &[Notification],
) -> Result<NotificationListBody, ApiError> {
    Ok(NotificationListBody {
        notifications: notifications
            .iter()
            .map(notification_body)
            .collect::<Result<_, _>>()?,
    })
}

fn notification_body(notification: &Notification) -> Result<NotificationBody, ApiError> {
    Ok(NotificationBody {
        notification_id: notification_id_uuid::format(notification.id()),
        kind: notification_kind_to_contract(notification.kind()),
        body: notification_text(&notification.push()),
        created_at: date_time_to_contract(notification.created_at())?,
        is_read: notification.is_read(),
    })
}

/// student-app・admin-web の `NotificationKind` の値（student-app の受け取り設定の項目名と同じ綴り）。各契約の enum には
/// その利用者が受け取る種類だけを載せる（添削の依頼はコーチだけ、添削の返却は生徒だけが受け取る）。
const fn notification_kind_to_contract(kind: NotificationKind) -> &'static str {
    match kind {
        NotificationKind::BookingConfirmed => "booking_confirmed",
        NotificationKind::VideoReviewRequested => "video_review_requested",
        NotificationKind::VideoReviewReturned => "video_review_returned",
        NotificationKind::MessageReceived => "message_received",
    }
}

/// お知らせの本文。受け取る側（生徒・コーチ）に合わせた文言にする。
const fn notification_text(push: &PushNotification) -> &'static str {
    match push {
        PushNotification::BookingConfirmed { .. } => "予約が確定しました",
        PushNotification::BookingReceived { .. } => "新しい予約が入りました",
        PushNotification::VideoReviewRequested { .. } => "生徒から動画の添削の依頼が届きました",
        PushNotification::VideoReviewReturned { .. } => "動画の添削が返ってきました",
        PushNotification::MessageReceived {
            sender: MessageSender::Coach,
            ..
        } => "コーチからメッセージが届きました",
        PushNotification::MessageReceived {
            sender: MessageSender::Student,
            ..
        } => "生徒からメッセージが届きました",
    }
}
