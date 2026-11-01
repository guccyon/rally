//! notification の値と、DB に保存する文字列の対応表。
//!
//! `notification.notification_settings.kind` と `notification.notifications.kind`（CHECK 制約の値）はこの表だけを通す。
//! CHECK との一致は `tests/schema.rs` が確かめる。

use domain::notification::{NotificationKind, PushNotification};

/// `notification.notification_settings.kind` の値。
pub const fn notification_kind_value(kind: NotificationKind) -> &'static str {
    match kind {
        NotificationKind::BookingConfirmed => "booking_confirmed",
        NotificationKind::VideoReviewRequested => "video_review_requested",
        NotificationKind::VideoReviewReturned => "video_review_returned",
        NotificationKind::MessageReceived => "message_received",
        NotificationKind::Announcement => "announcement",
    }
}

pub fn notification_kind_from_value(value: &str) -> Option<NotificationKind> {
    NotificationKind::ALL
        .into_iter()
        .find(|kind| notification_kind_value(*kind) == value)
}

/// `notification.notifications.kind` の値（[`push_notification_kind_value`]）。保存先の実装が読んだ行を変種に戻すのに使う。
pub const BOOKING_CONFIRMED: &str = "booking_confirmed";
pub const BOOKING_RECEIVED: &str = "booking_received";
pub const VIDEO_REVIEW_REQUESTED: &str = "video_review_requested";
pub const VIDEO_REVIEW_RETURNED: &str = "video_review_returned";
pub const MESSAGE_RECEIVED: &str = "message_received";
pub const ANNOUNCEMENT: &str = "announcement";

/// `notification.notifications.kind` のすべての値（CHECK 制約の値の順）。
pub const PUSH_NOTIFICATION_KIND_VALUES: [&str; 6] = [
    BOOKING_CONFIRMED,
    BOOKING_RECEIVED,
    VIDEO_REVIEW_REQUESTED,
    VIDEO_REVIEW_RETURNED,
    MESSAGE_RECEIVED,
    ANNOUNCEMENT,
];

/// `notification.notifications.kind` の値（プッシュ通知の変種ごと）。
pub const fn push_notification_kind_value(notification: &PushNotification) -> &'static str {
    match notification {
        PushNotification::BookingConfirmed { .. } => BOOKING_CONFIRMED,
        PushNotification::BookingReceived { .. } => BOOKING_RECEIVED,
        PushNotification::VideoReviewRequested { .. } => VIDEO_REVIEW_REQUESTED,
        PushNotification::VideoReviewReturned { .. } => VIDEO_REVIEW_RETURNED,
        PushNotification::MessageReceived { .. } => MESSAGE_RECEIVED,
        PushNotification::Announcement { .. } => ANNOUNCEMENT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_kind() {
        for kind in NotificationKind::ALL {
            assert_eq!(
                notification_kind_from_value(notification_kind_value(kind)),
                Some(kind)
            );
        }
        assert_eq!(notification_kind_from_value("booking_cancelled"), None);
    }
}
