//! notification: 通知（specs/context-map.yaml）。
//!
//! 生徒・コーチへのプッシュ通知（[`PushNotification`]）。送るきっかけは境界間イベントで、notification は
//! 他のコンテキストから直接呼ばれない。Gen1 は保存せず、送るだけ（送り先の基盤はモック）。表示する文言は
//! 境界のアダプタが決める。

mod push_notification;

pub use push_notification::PushNotification;
