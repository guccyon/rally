use std::cmp::Reverse;

use domain::{identity::AccountId, messaging::MessageThread};

use super::MessageThreadRepository;
use crate::Unavailable;

/// スレッドの一覧の入力。`account_id` は認証したアカウント。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListMessageThreadsQuery {
    pub account_id: AccountId,
}

#[derive(Debug)]
pub enum ListMessageThreadsError {
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// スレッドの一覧のユースケース（student-app・admin-web の `listMessageThreads`）。照会だけで、何も保存しない。
///
/// アカウントが当事者（生徒か担当コーチ）のスレッドを、最後の出来事（最後のメッセージの送信、なければスレッドを
/// 開いた時刻）の新しい順に返す。同じ時刻はスレッド ID の順。当事者のスレッドがなければ空。
pub struct ListMessageThreads<T> {
    threads: T,
}

impl<T> ListMessageThreads<T>
where
    T: MessageThreadRepository,
{
    pub fn new(threads: T) -> Self {
        Self { threads }
    }

    pub async fn execute(
        &self,
        query: ListMessageThreadsQuery,
    ) -> Result<Vec<MessageThread>, ListMessageThreadsError> {
        let mut threads = self
            .threads
            .list_for(query.account_id)
            .await
            .map_err(ListMessageThreadsError::Unavailable)?;
        threads.retain(|thread| thread.ensure_visible_to(query.account_id).is_ok());
        threads.sort_by_key(|thread| (Reverse(thread.last_activity_at()), thread.id()));
        Ok(threads)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::messaging::{MessageBody, MessageContent, MessageId, MessageThreadId};

    use super::*;
    use crate::in_memory::InMemoryMessageThreadRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn thread(id: u128, student: AccountId, coach: AccountId, opened: u64) -> MessageThread {
        MessageThread::open(MessageThreadId::from_u128(id), student, coach, at(opened)).unwrap()
    }

    fn ids(threads: &[MessageThread]) -> Vec<u128> {
        threads.iter().map(|t| t.id().as_u128()).collect()
    }

    #[tokio::test]
    async fn lists_the_threads_of_the_account_by_their_last_activity() {
        let mut talked = thread(10, STUDENT, COACH, 0);
        talked
            .send(
                MessageId::from_u128(1),
                COACH,
                MessageContent::Text(MessageBody::new("こんにちは").unwrap()),
                true,
                at(30),
            )
            .unwrap();
        let use_case = ListMessageThreads::new(InMemoryMessageThreadRepository::with_threads([
            talked,
            thread(11, STUDENT, OTHER_COACH, 20),
            thread(12, OTHER_STUDENT, COACH, 20),
            thread(13, OTHER_STUDENT, OTHER_COACH, 40),
        ]));
        let list = |account_id| use_case.execute(ListMessageThreadsQuery { account_id });

        assert_eq!(ids(&list(STUDENT).await.unwrap()), [10, 11]);
        // 最後の出来事が同じなら ID の順
        assert_eq!(ids(&list(COACH).await.unwrap()), [10, 12]);
        assert!(list(AccountId::from_u128(99)).await.unwrap().is_empty());
    }
}
