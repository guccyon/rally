//! messaging の値と、DB に保存する文字列の対応表。
//!
//! `messaging.messages.sender_role`（CHECK 制約の値）と、行動イベント `message.sent` の `sender_role` はこの表だけを
//! 通す。CHECK との一致は `tests/schema.rs` が確かめる。

use domain::messaging::MessageSender;

/// `messaging.messages.sender_role` の値（行動イベントの `sender_role` も同じ値）。
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
    }
}
