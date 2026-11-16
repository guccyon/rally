use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
};

use domain::{identity::AccountId, messaging::MessageThread};

use super::{MessageThreadList, MessageThreadRepository, ThreadStudentDisplayNameLookup};
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
/// アカウントが当事者（生徒か担当コーチ）のスレッドを返す。当事者のスレッドがなければ空。並べ方は次の順:
///
/// 1. アカウントが担当コーチのスレッドのうち、未返信（最後の発言が生徒のもの。
///    [`MessageThread::unanswered_student_message_at`]）のものを上に、生徒の最後の発言の新しい順
/// 2. 残りを最後の出来事（最後のメッセージの送信、なければスレッドを開いた時刻）の新しい順
/// 3. 同じならスレッド ID の順
///
/// 生徒が呼んだとき（未返信はコーチから見たもの）と未返信のスレッドがないときは、最後の出来事の新しい順だけになる。
///
/// 担当コーチとして呼んだとき（コーチ側のスレッドがあるとき）だけ、そのスレッドの生徒の表示名
/// （[`ThreadStudentDisplayNameLookup`]）を引いて添える。アカウントが引けない生徒は載せない。生徒が呼んだときは引かない。
pub struct ListMessageThreads<T, S> {
    threads: T,
    student_names: S,
}

impl<T, S> ListMessageThreads<T, S>
where
    T: MessageThreadRepository,
    S: ThreadStudentDisplayNameLookup,
{
    pub fn new(threads: T, student_names: S) -> Self {
        Self {
            threads,
            student_names,
        }
    }

    pub async fn execute(
        &self,
        query: ListMessageThreadsQuery,
    ) -> Result<MessageThreadList, ListMessageThreadsError> {
        let mut threads = self
            .threads
            .list_for(query.account_id)
            .await
            .map_err(ListMessageThreadsError::Unavailable)?;
        threads.retain(|thread| thread.ensure_visible_to(query.account_id).is_ok());
        threads.sort_by_key(|thread| {
            // Some（未返信）が None より先、新しい時刻が先
            let unanswered_at = (thread.coach_id() == query.account_id)
                .then(|| thread.unanswered_student_message_at())
                .flatten();
            (
                Reverse(unanswered_at),
                Reverse(thread.last_activity_at()),
                thread.id(),
            )
        });
        let student_ids: BTreeSet<AccountId> = threads
            .iter()
            .filter(|thread| thread.coach_id() == query.account_id)
            .map(MessageThread::student_id)
            .collect();
        let student_display_names = if student_ids.is_empty() {
            BTreeMap::new()
        } else {
            self.student_names
                .find_student_display_names(&student_ids)
                .await
                .map_err(ListMessageThreadsError::Unavailable)?
        };
        Ok(MessageThreadList {
            threads,
            student_display_names,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        messaging::{MessageBody, MessageContent, MessageId, MessageThreadId},
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::InMemoryMessageThreadRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    /// `STUDENT` の表示名だけを知っている。
    #[derive(Clone, Copy)]
    struct Names;

    impl ThreadStudentDisplayNameLookup for Names {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(student_ids
                .iter()
                .filter(|id| **id == STUDENT)
                .map(|id| (*id, DisplayName::new("あおき").unwrap()))
                .collect())
        }
    }

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn thread(id: u128, student: AccountId, coach: AccountId, opened: u64) -> MessageThread {
        MessageThread::open(MessageThreadId::from_u128(id), student, coach, at(opened)).unwrap()
    }

    fn send(thread: &mut MessageThread, id: u128, sender: AccountId, minute: u64) {
        thread
            .send(
                MessageId::from_u128(id),
                sender,
                MessageContent::Text(MessageBody::new("こんにちは").unwrap()),
                true,
                at(minute),
            )
            .unwrap();
    }

    fn ids(list: &MessageThreadList) -> Vec<u128> {
        list.threads.iter().map(|t| t.id().as_u128()).collect()
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
        let use_case = ListMessageThreads::new(
            InMemoryMessageThreadRepository::with_threads([
                talked,
                thread(11, STUDENT, OTHER_COACH, 20),
                thread(12, OTHER_STUDENT, COACH, 20),
                thread(13, OTHER_STUDENT, OTHER_COACH, 40),
            ]),
            Names,
        );
        let list = |account_id| use_case.execute(ListMessageThreadsQuery { account_id });

        let as_student = list(STUDENT).await.unwrap();
        assert_eq!(ids(&as_student), [10, 11]);
        // 生徒が呼んだときは表示名を引かない
        assert!(as_student.student_display_names.is_empty());
        // 最後の出来事が同じなら ID の順
        let as_coach = list(COACH).await.unwrap();
        assert_eq!(ids(&as_coach), [10, 12]);
        // 引けない生徒（OTHER_STUDENT）は載せない
        assert_eq!(
            as_coach
                .student_display_names
                .iter()
                .map(|(id, name)| (*id, name.as_str()))
                .collect::<Vec<_>>(),
            [(STUDENT, "あおき")]
        );
        assert!(
            list(AccountId::from_u128(99))
                .await
                .unwrap()
                .threads
                .is_empty()
        );
    }

    #[tokio::test]
    async fn the_coach_sees_unanswered_threads_first_by_the_last_student_message() {
        // 未返信: 生徒が 10 分に送った
        let mut asked_earlier = thread(10, STUDENT, COACH, 0);
        send(&mut asked_earlier, 1, STUDENT, 10);
        // 未返信: 生徒が 20 分に送った
        let mut asked_later = thread(11, OTHER_STUDENT, COACH, 0);
        send(&mut asked_later, 2, COACH, 5);
        send(&mut asked_later, 3, OTHER_STUDENT, 20);
        // 返信済み: 最後の出来事はいちばん新しいが、未返信より下
        let mut answered = thread(12, AccountId::from_u128(3), COACH, 0);
        send(&mut answered, 4, AccountId::from_u128(3), 25);
        send(&mut answered, 5, COACH, 30);
        // 発言がまだない
        let quiet = thread(13, AccountId::from_u128(4), COACH, 15);
        // 他のコーチの未返信のスレッドは返らない
        let mut others = thread(14, STUDENT, OTHER_COACH, 0);
        send(&mut others, 6, STUDENT, 40);
        let repository = InMemoryMessageThreadRepository::with_threads([
            asked_earlier,
            asked_later,
            answered,
            quiet,
            others,
        ]);
        let use_case = ListMessageThreads::new(repository.clone(), Names);
        let list = |account_id| use_case.execute(ListMessageThreadsQuery { account_id });

        let threads = list(COACH).await.unwrap();
        assert_eq!(ids(&threads), [11, 10, 12, 13]);
        assert_eq!(
            threads
                .threads
                .iter()
                .map(MessageThread::is_unanswered)
                .collect::<Vec<_>>(),
            [true, true, false, false]
        );

        // 生徒の一覧は未返信で並べ替えない（最後の出来事の新しい順のまま）
        assert_eq!(ids(&list(STUDENT).await.unwrap()), [14, 10]);

        // コーチが返すと未返信でなくなり、最後の出来事の順に戻る
        repository
            .update(MessageThreadId::from_u128(10), |thread| {
                thread
                    .send(
                        MessageId::from_u128(7),
                        COACH,
                        MessageContent::Text(MessageBody::new("お答えします").unwrap()),
                        true,
                        at(50),
                    )
                    .map(|_| ())
            })
            .await
            .unwrap();
        let threads = list(COACH).await.unwrap();
        assert_eq!(ids(&threads), [11, 10, 12, 13]);
        assert!(!threads.threads[1].is_unanswered());
        // 生徒がまた送ると上に戻る
        repository
            .update(MessageThreadId::from_u128(12), |thread| {
                thread
                    .send(
                        MessageId::from_u128(8),
                        AccountId::from_u128(3),
                        MessageContent::Text(MessageBody::new("追加で").unwrap()),
                        true,
                        at(60),
                    )
                    .map(|_| ())
            })
            .await
            .unwrap();
        assert_eq!(ids(&list(COACH).await.unwrap()), [12, 11, 10, 13]);
    }
}
