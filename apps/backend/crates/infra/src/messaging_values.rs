//! messaging の値と、DB に保存する文字列の対応表。
//!
//! `messaging.messages.sender_role` と `kind`（CHECK 制約の値）と、行動イベント `message.sent`・境界間イベント `MessageSent`
//! の `sender_role` はこの表だけを通す。CHECK との一致は `tests/schema.rs` が確かめる。

use domain::messaging::{MessageAuthor, MessageKind, MessageSender};

/// 当事者の値（`messaging.messages.sender_role` の当事者の値。行動イベント・境界間イベントの `sender_role` も同じ値）。
pub const fn message_sender_value(sender: MessageSender) -> &'static str {
    match sender {
        MessageSender::Student => "student",
        MessageSender::Coach => "coach",
    }
}

pub fn message_sender_from_value(value: &str) -> Option<MessageSender> {
    MessageSender::ALL
        .into_iter()
        .find(|sender| message_sender_value(*sender) == value)
}

/// `messaging.messages.sender_role` の値（当事者は [`message_sender_value`] と同じ。システムは `system`）。
pub const fn message_author_value(author: MessageAuthor) -> &'static str {
    match author {
        MessageAuthor::Participant(sender) => message_sender_value(sender),
        MessageAuthor::System => "system",
    }
}

pub fn message_author_from_value(value: &str) -> Option<MessageAuthor> {
    MessageAuthor::ALL
        .into_iter()
        .find(|author| message_author_value(*author) == value)
}

/// `messaging.messages.kind` の値。
pub const fn message_kind_value(kind: MessageKind) -> &'static str {
    match kind {
        MessageKind::Text => "text",
        MessageKind::VideoReviewRequest => "video_review_request",
        MessageKind::VideoReviewReturn => "video_review_return",
    }
}

pub fn message_kind_from_value(value: &str) -> Option<MessageKind> {
    MessageKind::ALL
        .into_iter()
        .find(|kind| message_kind_value(*kind) == value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_sender() {
        for sender in MessageSender::ALL {
            assert_eq!(
                message_sender_from_value(message_sender_value(sender)),
                Some(sender)
            );
        }
        assert_eq!(message_sender_from_value("headquarters"), None);
        assert_eq!(message_sender_from_value("system"), None, "当事者でない");
    }

    #[test]
    fn round_trips_every_author() {
        for author in MessageAuthor::ALL {
            assert_eq!(
                message_author_from_value(message_author_value(author)),
                Some(author)
            );
        }
        assert_eq!(message_author_value(MessageAuthor::System), "system");
        assert_eq!(message_author_from_value("headquarters"), None);
    }

    #[test]
    fn round_trips_every_kind() {
        for kind in MessageKind::ALL {
            assert_eq!(
                message_kind_from_value(message_kind_value(kind)),
                Some(kind)
            );
        }
        assert_eq!(message_kind_from_value("stamp"), None);
    }
}
