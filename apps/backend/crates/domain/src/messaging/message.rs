use std::time::SystemTime;

use super::{MessageBody, MessageId, MessageSender};
use crate::DomainError;

/// スレッド（[`super::MessageThread`]）の中の 1 通のメッセージ（glossary の message）。
///
/// 送った当事者（[`MessageSender`]）・本文・送信時刻と、受け取った側が読んだ時刻（既読。読むまでは `None`）を持つ。
/// 送信と既読は集約の [`super::MessageThread`] を通して行う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    id: MessageId,
    sender: MessageSender,
    body: MessageBody,
    sent_at: SystemTime,
    read_at: Option<SystemTime>,
}

impl Message {
    pub(super) const fn sent(
        id: MessageId,
        sender: MessageSender,
        body: MessageBody,
        sent_at: SystemTime,
    ) -> Self {
        Self {
            id,
            sender,
            body,
            sent_at,
            read_at: None,
        }
    }

    /// 保存先から読んだメッセージを組み立てる（保存先の実装用）。既読が送信より前なら
    /// [`DomainError::MessageReadBeforeSent`]。
    pub fn restore(
        id: MessageId,
        sender: MessageSender,
        body: MessageBody,
        sent_at: SystemTime,
        read_at: Option<SystemTime>,
    ) -> Result<Self, DomainError> {
        if read_at.is_some_and(|read_at| read_at < sent_at) {
            return Err(DomainError::MessageReadBeforeSent);
        }
        Ok(Self {
            id,
            sender,
            body,
            sent_at,
            read_at,
        })
    }

    /// `now` に既読にする。すでに読んでいれば最初に読んだ時刻のままで、`now` が送信より前なら読まない。
    /// 既読にしたら `true`。
    pub(super) fn read(&mut self, now: SystemTime) -> bool {
        if self.read_at.is_some() || now < self.sent_at {
            return false;
        }
        self.read_at = Some(now);
        true
    }

    pub const fn id(&self) -> MessageId {
        self.id
    }

    pub const fn sender(&self) -> MessageSender {
        self.sender
    }

    pub const fn body(&self) -> &MessageBody {
        &self.body
    }

    pub const fn sent_at(&self) -> SystemTime {
        self.sent_at
    }

    /// 受け取った側が読んだ時刻。読むまでは `None`。
    pub const fn read_at(&self) -> Option<SystemTime> {
        self.read_at
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const NOW: SystemTime = SystemTime::UNIX_EPOCH;

    fn body() -> MessageBody {
        MessageBody::new("よろしくお願いします").unwrap()
    }

    #[test]
    fn restore_rejects_reading_before_sending() {
        let later = NOW + Duration::from_secs(1);
        assert_eq!(
            Message::restore(
                MessageId::from_u128(1),
                MessageSender::Student,
                body(),
                later,
                Some(NOW)
            ),
            Err(DomainError::MessageReadBeforeSent)
        );
        for read_at in [None, Some(later), Some(later + Duration::from_secs(1))] {
            let message = Message::restore(
                MessageId::from_u128(1),
                MessageSender::Coach,
                body(),
                later,
                read_at,
            )
            .unwrap();
            assert_eq!(message.read_at(), read_at);
        }
    }

    #[test]
    fn reading_keeps_the_first_time_and_ignores_times_before_sending() {
        let sent_at = NOW + Duration::from_secs(10);
        let mut message = Message::sent(
            MessageId::from_u128(1),
            MessageSender::Student,
            body(),
            sent_at,
        );
        assert!(!message.read(NOW), "送信より前には読めない");
        assert_eq!(message.read_at(), None);
        assert!(message.read(sent_at), "送信と同じ時刻には読める");
        assert!(!message.read(sent_at + Duration::from_secs(5)));
        assert_eq!(message.read_at(), Some(sent_at));
    }
}
