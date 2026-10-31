use domain::{
    identity::AccountId,
    messaging::{MessageThread, MessageThreadId, MessagingError},
};

use super::{MessageThreadRepository, UpdateMessageThreadError};
use crate::clock::Clock;

/// 既読にする入力。`reader_id` は認証したアカウント（スレッドの当事者かは domain が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkMessageThreadReadCommand {
    pub reader_id: AccountId,
    pub thread_id: MessageThreadId,
}

#[derive(Debug)]
pub enum MarkMessageThreadReadError {
    /// スレッドがない
    ThreadNotFound,
    /// スレッドの当事者でない（[`MessagingError::NotAThreadParticipant`]）
    Invalid(MessagingError),
    /// 保存先の障害
    Unavailable(crate::Unavailable),
}

/// 既読のユースケース（student-app・admin-web の `markMessageThreadRead`。スレッドの当事者だけ）。
///
/// domain の [`MessageThread::mark_read`] で、相手から届いたメッセージを今の時刻で既読にして保存する。すでに
/// 読んだメッセージは最初に読んだ時刻のまま（何度呼んでもよい）。行動イベントは発火しない。
pub struct MarkMessageThreadRead<T, C> {
    threads: T,
    clock: C,
}

impl<T, C> MarkMessageThreadRead<T, C>
where
    T: MessageThreadRepository,
    C: Clock,
{
    pub fn new(threads: T, clock: C) -> Self {
        Self { threads, clock }
    }

    pub async fn execute(
        &self,
        command: MarkMessageThreadReadCommand,
    ) -> Result<MessageThread, MarkMessageThreadReadError> {
        let now = self.clock.now();
        self.threads
            .update(command.thread_id, |thread| {
                thread.mark_read(command.reader_id, now).map(|_| ())
            })
            .await
            .map_err(|error| match error {
                UpdateMessageThreadError::NotFound => MarkMessageThreadReadError::ThreadNotFound,
                UpdateMessageThreadError::Rejected(error) => {
                    MarkMessageThreadReadError::Invalid(error)
                }
                UpdateMessageThreadError::Unavailable(error) => {
                    MarkMessageThreadReadError::Unavailable(error)
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::messaging::{Message, MessageBody, MessageContent, MessageId};

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryMessageThreadRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const THREAD: MessageThreadId = MessageThreadId::from_u128(10);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn threads() -> InMemoryMessageThreadRepository {
        let mut thread = MessageThread::open(THREAD, STUDENT, COACH, at(0)).unwrap();
        for (id, sender) in [(1, STUDENT), (2, STUDENT), (3, COACH)] {
            thread
                .send(
                    MessageId::from_u128(id),
                    sender,
                    MessageContent::Text(MessageBody::new("x").unwrap()),
                    true,
                    at(id as u64),
                )
                .unwrap();
        }
        InMemoryMessageThreadRepository::with_threads([thread])
    }

    fn command(reader_id: AccountId, thread_id: MessageThreadId) -> MarkMessageThreadReadCommand {
        MarkMessageThreadReadCommand {
            reader_id,
            thread_id,
        }
    }

    #[tokio::test]
    async fn the_coach_reads_the_messages_of_the_student() {
        let threads = threads();
        let thread = MarkMessageThreadRead::new(threads.clone(), FixedClock(at(10)))
            .execute(command(COACH, THREAD))
            .await
            .unwrap();
        let read_at: Vec<_> = thread.messages().iter().map(Message::read_at).collect();
        assert_eq!(read_at, [Some(at(10)), Some(at(10)), None]);
        assert_eq!(thread.unread_count_for(COACH), 0);
        assert_eq!(threads.all(), vec![thread]);

        // 2 回目は最初に読んだ時刻のまま
        let again = MarkMessageThreadRead::new(threads, FixedClock(at(20)))
            .execute(command(COACH, THREAD))
            .await
            .unwrap();
        assert_eq!(again.messages()[0].read_at(), Some(at(10)));
    }

    #[tokio::test]
    async fn the_student_reads_the_messages_of_the_coach() {
        let threads = threads();
        let before = threads.all();
        assert_eq!(before[0].unread_count_for(STUDENT), 1);
        let thread = MarkMessageThreadRead::new(threads.clone(), FixedClock(at(10)))
            .execute(command(STUDENT, THREAD))
            .await
            .unwrap();
        let read_at: Vec<_> = thread.messages().iter().map(Message::read_at).collect();
        assert_eq!(read_at, [None, None, Some(at(10))]);
        assert_eq!(thread.unread_count_for(STUDENT), 0);
        assert_eq!(thread.unread_count_for(COACH), 2, "コーチの未読は変えない");
        assert_eq!(threads.all(), vec![thread]);
    }

    #[tokio::test]
    async fn others_and_unknown_threads_are_errors() {
        let threads = threads();
        let use_case = MarkMessageThreadRead::new(threads.clone(), FixedClock(at(10)));
        let before = threads.all();
        let result = use_case
            .execute(command(AccountId::from_u128(99), THREAD))
            .await;
        assert!(
            matches!(
                result,
                Err(MarkMessageThreadReadError::Invalid(
                    MessagingError::NotAThreadParticipant
                ))
            ),
            "{result:?}"
        );
        let result = use_case
            .execute(command(STUDENT, MessageThreadId::from_u128(11)))
            .await;
        assert!(
            matches!(result, Err(MarkMessageThreadReadError::ThreadNotFound)),
            "{result:?}"
        );
        assert_eq!(threads.all(), before);
    }
}
