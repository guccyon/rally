//! notification: 通知（specs/context-map.yaml）。
//!
//! 生徒・コーチへのプッシュ通知（[`PushNotification`]）と、通知の種類（[`NotificationKind`]）ごとに受け取るかを
//! 決める受け取り設定（[`NotificationSettings`]。既定はすべてオン）。送るきっかけは境界間イベントで、notification は
//! 他のコンテキストから直接呼ばれない（送り先の基盤はモック）。送ったプッシュ通知はお知らせ（[`Notification`]）として
//! 残し、受け取り設定でオフの種類もお知らせには残す（オフが止めるのはプッシュ通知だけ）。表示する文言は境界のアダプタが
//! 決める。

#[allow(clippy::module_inception)]
mod notification;
mod notification_id;
mod notification_kind;
mod notification_settings;
mod push_notification;

pub use notification::Notification;
pub use notification_id::NotificationId;
pub use notification_kind::NotificationKind;
pub use notification_settings::NotificationSettings;
pub use push_notification::PushNotification;
