//! notification のユースケースとポート。
//!
//! 境界間イベントの購読でプッシュ通知を送ってお知らせを残すもの（[`PushNotificationSubscriber`]）のほか、本部が生徒に
//! お知らせを送る（[`SendAnnouncement`]）・送った記録を一覧で見る（[`ListAnnouncements`]）。本部からのお知らせを受け取る
//! 生徒は profile に問い合わせる（[`AnnouncementRecipientLookup`]）。

pub mod announcement_id_uuid;
mod announcement_recipient_lookup;
mod announcement_repository;
mod get_notification_settings;
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
mod update_notification_settings;

pub use announcement_recipient_lookup::AnnouncementRecipientLookup;
pub use announcement_repository::AnnouncementRepository;
pub use get_notification_settings::GetNotificationSettings;
pub use list_announcements::ListAnnouncements;
pub use list_my_notifications::ListMyNotifications;
pub use mark_my_notifications_read::MarkMyNotificationsRead;
pub use my_notification::MyNotification;
pub use notification_repository::NotificationRepository;
pub use push_notification_subscriber::PushNotificationSubscriber;
pub use push_sender::PushSender;
pub use repository::NotificationSettingsRepository;
pub use send_announcement::{SendAnnouncement, SendAnnouncementCommand, SendAnnouncementError};
pub use update_notification_settings::{
    UpdateNotificationSettings, UpdateNotificationSettingsCommand,
};
