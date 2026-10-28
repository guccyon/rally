//! notification の値と、DB に保存する文字列の対応表。
//!
//! `notification.notification_settings.kind`（CHECK 制約の値）はこの表だけを通す。CHECK との一致は `tests/schema.rs` が
//! 確かめる。

use domain::notification::NotificationKind;

/// `notification.notification_settings.kind` の値。
pub const fn notification_kind_value(kind: NotificationKind) -> &'static str {
    match kind {
        NotificationKind::BookingConfirmed => "booking_confirmed",
        NotificationKind::VideoReviewReturned => "video_review_returned",
        NotificationKind::MessageReceived => "message_received",
    }
}

pub fn notification_kind_from_value(value: &str) -> Option<NotificationKind> {
    NotificationKind::ALL
        .into_iter()
        .find(|kind| notification_kind_value(*kind) == value)
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
