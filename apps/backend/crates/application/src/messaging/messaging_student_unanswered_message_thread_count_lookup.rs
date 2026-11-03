use std::{
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};

use domain::{identity::AccountId, messaging::StudentUnansweredMessageThreadCount};

use super::MessageThreadRepository;
use crate::{
    Unavailable,
    identity::{
        StudentUnansweredMessageThreadCountLookup, StudentUnansweredMessageThreadCountsLookup,
    },
};

/// identity の [`StudentUnansweredMessageThreadCountLookup`] と [`StudentUnansweredMessageThreadCountsLookup`] の
/// messaging による実装。生徒 1 人ならスレッドの保存先から生徒が当事者のスレッドを読み
/// （[`MessageThreadRepository::list_for`]。`ListUnansweredMessageThreads` に `student_id` を付けたときと同じ読み方）、
/// 複数の生徒ならすべてのスレッドを 1 回で読み（[`MessageThreadRepository::list_all`]。`ListUnansweredMessageThreads` と
/// 同じ読み方）生徒ごとに分けて、照会した時刻で domain の [`StudentUnansweredMessageThreadCount::tally`] で数える。
/// 何も変えない（既読にしない）。
#[derive(Debug, Clone)]
pub struct MessagingStudentUnansweredMessageThreadCountLookup<R> {
    threads: R,
}

impl<R> MessagingStudentUnansweredMessageThreadCountLookup<R> {
    pub fn new(threads: R) -> Self {
        Self { threads }
    }
}

impl<R> StudentUnansweredMessageThreadCountLookup
    for MessagingStudentUnansweredMessageThreadCountLookup<R>
where
    R: MessageThreadRepository,
{
    async fn find_unanswered_message_thread_count(
        &self,
        student_id: AccountId,
        now: SystemTime,
    ) -> Result<StudentUnansweredMessageThreadCount, Unavailable> {
        let threads = self.threads.list_for(student_id).await?;
        Ok(StudentUnansweredMessageThreadCount::tally(
            student_id, &threads, now,
        ))
    }
}

impl<R> StudentUnansweredMessageThreadCountsLookup
    for MessagingStudentUnansweredMessageThreadCountLookup<R>
where
    R: MessageThreadRepository,
{
    async fn find_unanswered_message_thread_counts(
        &self,
        student_ids: &BTreeSet<AccountId>,
        now: SystemTime,
    ) -> Result<BTreeMap<AccountId, StudentUnansweredMessageThreadCount>, Unavailable> {
        if student_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut threads_of: BTreeMap<AccountId, Vec<_>> =
            student_ids.iter().map(|id| (*id, Vec::new())).collect();
        for thread in self.threads.list_all().await? {
            if let Some(threads) = threads_of.get_mut(&thread.student_id()) {
                threads.push(thread);
            }
        }
        Ok(threads_of
            .into_iter()
            .map(|(student_id, threads)| {
                (
                    student_id,
                    StudentUnansweredMessageThreadCount::tally(student_id, &threads, now),
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        time::Duration,
    };

    use domain::{
        messaging::{
            MessageBody, MessageContent, MessageId, MessageThread, MessageThreadId,
            UnansweredThreadCriteria,
        },
        profile::DisplayName,
    };

    use super::*;
    use crate::{
        in_memory::{FixedClock, InMemoryMessageThreadRepository},
        messaging::{
            ListUnansweredMessageThreads, ThreadCoachDisplayNameLookup,
            ThreadStudentDisplayNameLookup,
        },
    };

    const HOUR: Duration = Duration::from_secs(3600);
    const SECOND: Duration = Duration::from_secs(1);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const NO_THREADS_STUDENT: AccountId = AccountId::from_u128(3);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn now() -> SystemTime {
        // 2026-11-01T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    /// `id` の `student` と `coach` のスレッドを照会の 10 日前に開き、`exchange` の順に（生徒が送ったか, 送信時刻）で
    /// 本文を送る。
    fn thread(
        id: u128,
        student: AccountId,
        coach: AccountId,
        exchange: &[(bool, SystemTime)],
    ) -> MessageThread {
        let mut thread = MessageThread::open(
            MessageThreadId::from_u128(id),
            student,
            coach,
            now() - 240 * HOUR,
        )
        .unwrap();
        for (n, (by_student, at)) in exchange.iter().enumerate() {
            thread
                .send(
                    MessageId::from_u128(id * 100 + n as u128),
                    if *by_student { student } else { coach },
                    MessageContent::Text(MessageBody::new("本部には見せない本文").unwrap()),
                    true,
                    *at,
                )
                .unwrap();
        }
        thread
    }

    fn threads() -> InMemoryMessageThreadRepository {
        InMemoryMessageThreadRepository::with_threads([
            // 24 時間ちょうど前の生徒の発言
            thread(1, STUDENT, COACH, &[(true, now() - 24 * HOUR)]),
            // 別のコーチとのスレッドで、24 時間に 1 秒足りない
            thread(
                2,
                STUDENT,
                OTHER_COACH,
                &[(true, now() - 24 * HOUR + SECOND)],
            ),
            // 他の生徒の、72 時間前の生徒の発言
            thread(3, OTHER_STUDENT, COACH, &[(true, now() - 72 * HOUR)]),
        ])
    }

    /// 表示名を何も知らない（数には要らない）。
    #[derive(Clone)]
    struct NoNames;

    impl ThreadCoachDisplayNameLookup for NoNames {
        async fn find_coach_display_names(
            &self,
            _: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(BTreeMap::new())
        }
    }

    impl ThreadStudentDisplayNameLookup for NoNames {
        async fn find_student_display_names(
            &self,
            _: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(BTreeMap::new())
        }
    }

    #[tokio::test]
    async fn counts_unanswered_threads_of_the_student_with_the_default_hours() {
        let lookup = MessagingStudentUnansweredMessageThreadCountLookup::new(threads());
        let count = lookup
            .find_unanswered_message_thread_count(STUDENT, now())
            .await
            .unwrap();
        assert_eq!(count.student_id(), STUDENT);
        assert_eq!(
            count.unanswered_count(),
            1,
            "24 時間ちょうどは含み、それに満たないスレッド・他の生徒のスレッドは数えない"
        );

        // 時間は照会した時刻から数える（1 秒後には 24 時間に 1 秒足りなかったスレッドも数える）
        let later = lookup
            .find_unanswered_message_thread_count(STUDENT, now() + SECOND)
            .await
            .unwrap();
        assert_eq!(later.unanswered_count(), 2);

        let none = lookup
            .find_unanswered_message_thread_count(NO_THREADS_STUDENT, now())
            .await
            .unwrap();
        assert_eq!(none.unanswered_count(), 0);
    }

    #[tokio::test]
    async fn the_count_is_the_same_as_the_default_list_of_the_student() {
        let lookup = MessagingStudentUnansweredMessageThreadCountLookup::new(threads());
        for at in [now(), now() + SECOND, now() - 48 * HOUR] {
            for student_id in [STUDENT, OTHER_STUDENT, COACH, NO_THREADS_STUDENT] {
                let listed =
                    ListUnansweredMessageThreads::new(threads(), NoNames, NoNames, FixedClock(at))
                        .execute(
                            UnansweredThreadCriteria::default().with_student_id(Some(student_id)),
                        )
                        .await
                        .unwrap()
                        .threads
                        .len();
                let count = lookup
                    .find_unanswered_message_thread_count(student_id, at)
                    .await
                    .unwrap();
                assert_eq!(
                    count.unanswered_count() as usize,
                    listed,
                    "{student_id:?} {at:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn counts_of_the_asked_students_are_the_same_as_each_count() {
        let lookup = MessagingStudentUnansweredMessageThreadCountLookup::new(threads());
        let asked: BTreeSet<AccountId> = [STUDENT, NO_THREADS_STUDENT, COACH].into();
        for at in [now(), now() + SECOND, now() - 48 * HOUR] {
            let counts = lookup
                .find_unanswered_message_thread_counts(&asked, at)
                .await
                .unwrap();
            assert_eq!(
                counts.keys().copied().collect::<BTreeSet<_>>(),
                asked,
                "聞いた生徒だけをすべて返す（スレッドのない生徒も含め、聞いていない生徒 2 は含めない）"
            );
            for (student_id, count) in &counts {
                assert_eq!(count.student_id(), *student_id);
                let each = lookup
                    .find_unanswered_message_thread_count(*student_id, at)
                    .await
                    .unwrap();
                assert_eq!(*count, each, "{student_id:?} {at:?}");
            }
        }
        let counts = lookup
            .find_unanswered_message_thread_counts(&asked, now())
            .await
            .unwrap();
        assert_eq!(counts[&STUDENT].unanswered_count(), 1);
        assert_eq!(counts[&NO_THREADS_STUDENT].unanswered_count(), 0);
        assert_eq!(
            counts[&COACH].unanswered_count(),
            0,
            "コーチの ID では数えない"
        );

        assert!(
            lookup
                .find_unanswered_message_thread_counts(&BTreeSet::new(), now())
                .await
                .unwrap()
                .is_empty()
        );
    }
}
