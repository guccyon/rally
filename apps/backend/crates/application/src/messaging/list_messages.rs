use domain::{
    DomainError,
    identity::AccountId,
    messaging::{MessageThread, MessageThreadId},
};

use super::MessageThreadRepository;
use crate::Unavailable;

/// メッセージの一覧の入力。`account_id` は認証したアカウント（スレッドの当事者かは domain が確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListMessagesQuery {
    pub account_id: AccountId,
    pub thread_id: MessageThreadId,
}

#[derive(Debug)]
pub enum ListMessagesError {
    /// スレッドがない
    ThreadNotFound,
    /// スレッドの当事者でない（[`DomainError::NotAThreadParticipant`]）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// メッセージの一覧のユースケース（student-app・admin-web の `listMessages`）。照会だけで、既読にもしない
/// （既読は [`super::MarkMessageThreadRead`]）。
///
/// スレッドを返す。メッセージは [`MessageThread::messages`]（送った順）。読めるのはスレッドの当事者だけ。
pub struct ListMessages<T> {
    threads: T,
}

impl<T> ListMessages<T>
where
    T: MessageThreadRepository,
{
    pub fn new(threads: T) -> Self {
        Self { threads }
    }

    pub async fn execute(
        &self,
        query: ListMessagesQuery,
    ) -> Result<MessageThread, ListMessagesError> {
        let thread = self
            .threads
            .find(query.thread_id)
            .await
            .map_err(ListMessagesError::Unavailable)?
            .ok_or(ListMessagesError::ThreadNotFound)?;
        thread
            .ensure_visible_to(query.account_id)
            .map_err(ListMessagesError::Invalid)?;
        Ok(thread)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use crate::in_memory::InMemoryMessageThreadRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const THREAD: MessageThreadId = MessageThreadId::from_u128(10);

    fn use_case() -> ListMessages<InMemoryMessageThreadRepository> {
        ListMessages::new(InMemoryMessageThreadRepository::with_threads([
            MessageThread::open(THREAD, STUDENT, COACH, SystemTime::UNIX_EPOCH).unwrap(),
        ]))
    }

    fn query(account_id: AccountId, thread_id: MessageThreadId) -> ListMessagesQuery {
        ListMessagesQuery {
            account_id,
            thread_id,
        }
    }

    #[tokio::test]
    async fn the_participants_read_the_thread() {
        let use_case = use_case();
        for account in [STUDENT, COACH] {
            let thread = use_case.execute(query(account, THREAD)).await.unwrap();
            assert_eq!(thread.id(), THREAD);
        }
    }

    #[tokio::test]
    async fn others_and_unknown_threads_are_errors() {
        let use_case = use_case();
        let result = use_case
            .execute(query(AccountId::from_u128(99), THREAD))
            .await;
        assert!(
            matches!(
                result,
                Err(ListMessagesError::Invalid(
                    DomainError::NotAThreadParticipant
                ))
            ),
            "{result:?}"
        );
        let result = use_case
            .execute(query(STUDENT, MessageThreadId::from_u128(11)))
            .await;
        assert!(
            matches!(result, Err(ListMessagesError::ThreadNotFound)),
            "{result:?}"
        );
    }
}
