use std::collections::{BTreeMap, BTreeSet};

use domain::{identity::AccountId, messaging::CoachUnansweredMessageThreadCount};

use super::MessageThreadRepository;
use crate::{
    Unavailable,
    clock::Clock,
    profile::{CoachUnansweredMessageThreadCountLookup, CoachUnansweredMessageThreadCountsLookup},
};

/// profile の [`CoachUnansweredMessageThreadCountLookup`] と [`CoachUnansweredMessageThreadCountsLookup`] の messaging
/// による実装。コーチ 1 人ならスレッドの保存先からコーチが当事者のスレッドを読み（[`MessageThreadRepository::list_for`]。
/// `ListUnansweredMessageThreads` に `coach_id` を付けたときと同じ読み方）、複数のコーチならすべてのスレッドを 1 回で読み
/// （[`MessageThreadRepository::list_all`]。`ListUnansweredMessageThreads` と同じ読み方）コーチごとに分けて、照会した時刻
/// （[`Clock`]）で domain の [`CoachUnansweredMessageThreadCount::tally`] で数える。何も変えない（既読にしない）。
#[derive(Debug, Clone)]
pub struct MessagingCoachUnansweredMessageThreadCountLookup<R, C> {
    threads: R,
    clock: C,
}

impl<R, C> MessagingCoachUnansweredMessageThreadCountLookup<R, C> {
    pub fn new(threads: R, clock: C) -> Self {
        Self { threads, clock }
    }
}

impl<R, C> CoachUnansweredMessageThreadCountLookup
    for MessagingCoachUnansweredMessageThreadCountLookup<R, C>
where
    R: MessageThreadRepository,
    C: Clock,
{
    async fn find_unanswered_message_thread_count(
        &self,
        coach_id: AccountId,
    ) -> Result<CoachUnansweredMessageThreadCount, Unavailable> {
        let now = self.clock.now();
        let threads = self.threads.list_for(coach_id).await?;
        Ok(CoachUnansweredMessageThreadCount::tally(
            coach_id, &threads, now,
        ))
    }
}

impl<R, C> CoachUnansweredMessageThreadCountsLookup
    for MessagingCoachUnansweredMessageThreadCountLookup<R, C>
where
    R: MessageThreadRepository,
    C: Clock,
{
    async fn find_unanswered_message_thread_counts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, CoachUnansweredMessageThreadCount>, Unavailable> {
        if coach_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let now = self.clock.now();
        let mut threads_of: BTreeMap<AccountId, Vec<_>> =
            coach_ids.iter().map(|id| (*id, Vec::new())).collect();
        for thread in self.threads.list_all().await? {
            if let Some(threads) = threads_of.get_mut(&thread.coach_id()) {
                threads.push(thread);
            }
        }
        Ok(threads_of
            .into_iter()
            .map(|(coach_id, threads)| {
                (
                    coach_id,
                    CoachUnansweredMessageThreadCount::tally(coach_id, &threads, now),
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

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
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const NO_THREADS_COACH: AccountId = AccountId::from_u128(9);

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
            // 24 時間に 1 秒足りない
            thread(
                2,
                OTHER_STUDENT,
                COACH,
                &[(true, now() - 24 * HOUR + SECOND)],
            ),
            // 他のコーチの、72 時間前の生徒の発言
            thread(3, STUDENT, OTHER_COACH, &[(true, now() - 72 * HOUR)]),
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
    async fn counts_unanswered_threads_of_the_coach_with_the_default_hours() {
        let count =
            MessagingCoachUnansweredMessageThreadCountLookup::new(threads(), FixedClock(now()))
                .find_unanswered_message_thread_count(COACH)
                .await
                .unwrap();
        assert_eq!(count.coach_id(), COACH);
        assert_eq!(
            count.unanswered_count(),
            1,
            "24 時間ちょうどは含み、それに満たないスレッド・他のコーチのスレッドは数えない"
        );

        // 時間は照会した時刻から数える（1 秒後には 24 時間に 1 秒足りなかったスレッドも数える）
        let later = MessagingCoachUnansweredMessageThreadCountLookup::new(
            threads(),
            FixedClock(now() + SECOND),
        )
        .find_unanswered_message_thread_count(COACH)
        .await
        .unwrap();
        assert_eq!(later.unanswered_count(), 2);

        let none =
            MessagingCoachUnansweredMessageThreadCountLookup::new(threads(), FixedClock(now()))
                .find_unanswered_message_thread_count(NO_THREADS_COACH)
                .await
                .unwrap();
        assert_eq!(none.unanswered_count(), 0);
    }

    #[tokio::test]
    async fn the_count_is_the_same_as_the_default_list_of_the_coach() {
        for at in [now(), now() + SECOND, now() - 48 * HOUR] {
            for coach_id in [COACH, OTHER_COACH, STUDENT, NO_THREADS_COACH] {
                let listed =
                    ListUnansweredMessageThreads::new(threads(), NoNames, NoNames, FixedClock(at))
                        .execute(UnansweredThreadCriteria::default().with_coach_id(Some(coach_id)))
                        .await
                        .unwrap()
                        .threads
                        .len();
                let count = MessagingCoachUnansweredMessageThreadCountLookup::new(
                    threads(),
                    FixedClock(at),
                )
                .find_unanswered_message_thread_count(coach_id)
                .await
                .unwrap();
                assert_eq!(
                    count.unanswered_count() as usize,
                    listed,
                    "{coach_id:?} {at:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn counts_of_the_asked_coaches_are_the_same_as_each_count() {
        let asked: BTreeSet<AccountId> = [COACH, NO_THREADS_COACH, STUDENT].into();
        for at in [now(), now() + SECOND, now() - 48 * HOUR] {
            let lookup =
                MessagingCoachUnansweredMessageThreadCountLookup::new(threads(), FixedClock(at));
            let counts = lookup
                .find_unanswered_message_thread_counts(&asked)
                .await
                .unwrap();
            assert_eq!(
                counts.keys().copied().collect::<BTreeSet<_>>(),
                asked,
                "聞いたコーチだけをすべて返す（スレッドのないコーチも含め、聞いていないコーチ 8 は含めない）"
            );
            for (coach_id, count) in &counts {
                assert_eq!(count.coach_id(), *coach_id);
                let each = lookup
                    .find_unanswered_message_thread_count(*coach_id)
                    .await
                    .unwrap();
                assert_eq!(*count, each, "{coach_id:?} {at:?}");
            }
        }
        let counts =
            MessagingCoachUnansweredMessageThreadCountLookup::new(threads(), FixedClock(now()))
                .find_unanswered_message_thread_counts(&asked)
                .await
                .unwrap();
        assert_eq!(counts[&COACH].unanswered_count(), 1);
        assert_eq!(counts[&NO_THREADS_COACH].unanswered_count(), 0);
        assert_eq!(
            counts[&STUDENT].unanswered_count(),
            0,
            "生徒の ID では数えない"
        );

        assert!(
            MessagingCoachUnansweredMessageThreadCountLookup::new(threads(), FixedClock(now()))
                .find_unanswered_message_thread_counts(&BTreeSet::new())
                .await
                .unwrap()
                .is_empty()
        );
    }
}
