//! notification: 通知（specs/context-map.yaml）。
//!
//! 生徒・コーチへのプッシュ通知（[`PushNotification`]）と、通知の種類（[`NotificationKind`]）ごとに受け取るかを
//! 決める受け取り設定（[`NotificationSettings`]。既定はすべてオン）。送るきっかけは境界間イベントで、notification は
//! 他のコンテキストから直接呼ばれない（送り先の基盤はモック）。送ったプッシュ通知はお知らせ（[`Notification`]）として
//! 残し、受け取り設定でオフの種類もお知らせには残す（オフが止めるのはプッシュ通知だけ）。表示する文言は境界のアダプタが
//! 決める。
//!
//! 本部は生徒全員か、あるスポーツの生徒全員にお知らせを送れる（[`Announcement`]。送った記録）。受け取る生徒ごとに
//! お知らせを作り、プッシュ通知は受け取り設定に従う（種類は [`NotificationKind::Announcement`]）。誤って送ったお知らせは
//! 本部が取り下げられ（[`AnnouncementWithdrawal`]）、取り下げたお知らせは生徒のお知らせの一覧に出さない。
//! 本部は送る前のお知らせを下書き（[`AnnouncementDraft`]）として残し、あとで直してから送れる。下書きは生徒に届かない。

mod announcement;
mod announcement_body;
mod announcement_draft;
mod announcement_draft_id;
mod announcement_id;
mod announcement_title;
mod announcement_withdrawal;
#[allow(clippy::module_inception)]
mod notification;
mod notification_error;
mod notification_id;
mod notification_kind;
mod notification_settings;
mod push_notification;

pub use announcement::Announcement;
pub use announcement_body::AnnouncementBody;
pub use announcement_draft::AnnouncementDraft;
pub use announcement_draft_id::AnnouncementDraftId;
pub use announcement_id::AnnouncementId;
pub use announcement_title::AnnouncementTitle;
pub use announcement_withdrawal::AnnouncementWithdrawal;
pub use notification::Notification;
pub use notification_error::NotificationError;
pub use notification_id::NotificationId;
pub use notification_kind::NotificationKind;
pub use notification_settings::NotificationSettings;
pub use push_notification::PushNotification;
