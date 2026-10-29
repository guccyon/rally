use std::time::SystemTime;

use super::{MessageAuthor, MessageContent, MessageId};
use crate::DomainError;

/// スレッド（[`super::MessageThread`]）の中の 1 通のメッセージ（glossary の message）。
///
/// 流した送り手（[`MessageAuthor`]）・中身（[`MessageContent`]。本文・添削依頼・添削の返却）・送信時刻と、受け取った
/// 側が読んだ時刻（既読。読むまでは `None`）を持つ。中身の種類に合う送り手だけが流せる（[`MessageContent::allows`]）。
/// 送信と既読は集約の [`super::MessageThread`] を通して行う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    id: MessageId,
    author: MessageAuthor,
    content: MessageContent,
    sent_at: SystemTime,
    read_at: Option<SystemTime>,
}

impl Message {
    /// 送り手が中身の種類に合わなければ [`DomainError::MessageAuthorMismatchesKind`]。
    pub(super) fn sent(
        id: MessageId,
        author: MessageAuthor,
        content: MessageContent,
        sent_at: SystemTime,
    ) -> Result<Self, DomainError> {
        Self::restore(id, author, content, sent_at, None)
    }

    /// 保存先から読んだメッセージを組み立てる（保存先の実装用）。送り手が中身の種類に合わなければ
    /// [`DomainError::MessageAuthorMismatchesKind`]、既読が送信より前なら [`DomainError::MessageReadBeforeSent`]。
    pub fn restore(
        id: MessageId,
        author: MessageAuthor,
        content: MessageContent,
        sent_at: SystemTime,
        read_at: Option<SystemTime>,
    ) -> Result<Self, DomainError> {
        if !content.allows(author) {
            return Err(DomainError::MessageAuthorMismatchesKind);
        }
        if read_at.is_some_and(|read_at| read_at < sent_at) {
            return Err(DomainError::MessageReadBeforeSent);
        }
        Ok(Self {
            id,
            author,
            content,
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

    pub const fn author(&self) -> MessageAuthor {
        self.author
    }

    pub const fn content(&self) -> &MessageContent {
        &self.content
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
    use crate::{
        async_lesson::{VideoId, VideoReviewId},
        messaging::{MessageBody, MessageSender},
    };

    const NOW: SystemTime = SystemTime::UNIX_EPOCH;
    const STUDENT: MessageAuthor = MessageAuthor::Participant(MessageSender::Student);
    const COACH: MessageAuthor = MessageAuthor::Participant(MessageSender::Coach);

    fn text() -> MessageContent {
        MessageContent::Text(MessageBody::new("よろしくお願いします").unwrap())
    }

    #[test]
    fn restore_rejects_reading_before_sending() {
        let later = NOW + Duration::from_secs(1);
        assert_eq!(
            Message::restore(MessageId::from_u128(1), STUDENT, text(), later, Some(NOW)),
            Err(DomainError::MessageReadBeforeSent)
        );
        for read_at in [None, Some(later), Some(later + Duration::from_secs(1))] {
            let message =
                Message::restore(MessageId::from_u128(1), COACH, text(), later, read_at).unwrap();
            assert_eq!(message.read_at(), read_at);
        }
    }

    #[test]
    fn the_author_must_fit_the_kind() {
        let request = MessageContent::VideoReviewRequest {
            video_id: VideoId::from_u128(5),
        };
        let returned = MessageContent::VideoReviewReturn {
            video_id: VideoId::from_u128(5),
            review_id: VideoReviewId::from_u128(6),
        };
        for (author, content, ok) in [
            (STUDENT, request.clone(), true),
            (COACH, request.clone(), false),
            (MessageAuthor::System, request, false),
            (COACH, returned.clone(), true),
            (STUDENT, returned.clone(), false),
            (MessageAuthor::System, returned, false),
            (MessageAuthor::System, text(), true),
        ] {
            let result = Message::sent(MessageId::from_u128(1), author, content.clone(), NOW);
            if ok {
                let message = result.unwrap();
                assert_eq!((message.author(), message.content()), (author, &content));
            } else {
                assert_eq!(
                    result,
                    Err(DomainError::MessageAuthorMismatchesKind),
                    "{author:?} {content:?}"
                );
            }
        }
    }

    #[test]
    fn reading_keeps_the_first_time_and_ignores_times_before_sending() {
        let sent_at = NOW + Duration::from_secs(10);
        let mut message = Message::sent(MessageId::from_u128(1), STUDENT, text(), sent_at).unwrap();
        assert!(!message.read(NOW), "送信より前には読めない");
        assert_eq!(message.read_at(), None);
        assert!(message.read(sent_at), "送信と同じ時刻には読める");
        assert!(!message.read(sent_at + Duration::from_secs(5)));
        assert_eq!(message.read_at(), Some(sent_at));
    }
}
