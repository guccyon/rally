//! notification のユースケースとポート。
//!
//! 境界間イベントの購読でプッシュ通知を送ってお知らせを残すもの（[`PushNotificationSubscriber`]）のほか、本部が生徒に
//! お知らせを送る（[`SendAnnouncement`]）・送った記録を一覧で見る（[`ListAnnouncements`]）・誤って送ったものを取り下げる
//! （[`WithdrawAnnouncement`]。取り下げたものは生徒のお知らせの一覧に出さない）。本部からのお知らせを受け取る
//! 生徒は profile に問い合わせる（[`AnnouncementRecipientLookup`]）。
//!
//! 本部は送る前のお知らせを下書きとして残し（[`CreateAnnouncementDraft`]・[`UpdateAnnouncementDraft`]・
//! [`DeleteAnnouncementDraft`]・[`ListAnnouncementDrafts`]。保存先は [`AnnouncementDraftRepository`]）、下書きから送る
//! （[`SendAnnouncementDraft`]。送るのは [`SendAnnouncement`] と同じで、送ったら下書きを消す）。下書きは生徒に届かない。

mod announcement_draft_error;
pub mod announcement_draft_id_uuid;
mod announcement_draft_repository;
pub mod announcement_id_uuid;
mod announcement_recipient_lookup;
mod announcement_repository;
mod create_announcement_draft;
mod delete_announcement_draft;
mod get_notification_settings;
mod list_announcement_drafts;
mod list_announcements;
mod list_my_notifications;
mod mark_my_notifications_read;
mod my_notification;
pub mod notification_id_uuid;
mod notification_repository;
mod push_notification_subscriber;
mod push_sender;
mod repository;
mod send_announcement;
mod send_announcement_draft;
mod update_announcement_draft;
mod update_notification_settings;
mod withdraw_announcement;
mod withdraw_announcement_error;

pub use announcement_draft_error::AnnouncementDraftError;
pub use announcement_draft_repository::AnnouncementDraftRepository;
pub use announcement_recipient_lookup::AnnouncementRecipientLookup;
pub use announcement_repository::AnnouncementRepository;
pub use create_announcement_draft::CreateAnnouncementDraft;
pub use delete_announcement_draft::DeleteAnnouncementDraft;
pub use get_notification_settings::GetNotificationSettings;
pub use list_announcement_drafts::ListAnnouncementDrafts;
pub use list_announcements::ListAnnouncements;
pub use list_my_notifications::ListMyNotifications;
pub use mark_my_notifications_read::MarkMyNotificationsRead;
pub use my_notification::MyNotification;
pub use notification_repository::NotificationRepository;
pub use push_notification_subscriber::PushNotificationSubscriber;
pub use push_sender::PushSender;
pub use repository::NotificationSettingsRepository;
pub use send_announcement::{SendAnnouncement, SendAnnouncementCommand, SendAnnouncementError};
pub use send_announcement_draft::SendAnnouncementDraft;
pub use update_announcement_draft::UpdateAnnouncementDraft;
pub use update_notification_settings::{
    UpdateNotificationSettings, UpdateNotificationSettingsCommand,
};
pub use withdraw_announcement::WithdrawAnnouncement;
pub use withdraw_announcement_error::WithdrawAnnouncementError;
