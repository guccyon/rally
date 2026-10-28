//! notification: 通知（specs/context-map.yaml）。
//!
//! 生徒・コーチへのプッシュ通知（[`PushNotification`]）と、通知の種類（[`NotificationKind`]）ごとに受け取るかを
//! 決める受け取り設定（[`NotificationSettings`]。既定はすべてオン）。送るきっかけは境界間イベントで、notification は
//! 他のコンテキストから直接呼ばれない。通知そのものは保存せず、送るだけ（送り先の基盤はモック）。表示する文言は
//! 境界のアダプタが決める。

mod notification_kind;
mod notification_settings;
mod push_notification;

pub use notification_kind::NotificationKind;
pub use notification_settings::NotificationSettings;
pub use push_notification::PushNotification;
