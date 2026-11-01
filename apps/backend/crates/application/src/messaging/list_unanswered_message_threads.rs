use std::collections::{BTreeMap, BTreeSet};

use domain::{
    identity::AccountId,
    messaging::{MessageThread, UnansweredThreadCriteria},
};

use super::{
    MessageThreadRepository, ThreadCoachDisplayNameLookup, ThreadStudentDisplayNameLookup,
    UnansweredThreadList,
};
use crate::{Unavailable, clock::Clock};

/// 本部が見る、返信のないまま時間の過ぎたスレッドの一覧のユースケース（admin-web の `listUnansweredMessageThreads`。
/// 本部だけが呼ぶ）。照会だけで、何も保存しない（既読にしない・コーチに催促しない）。
///
/// すべてのスレッドを読み、domain の [`UnansweredThreadCriteria::select`] で、最後の発言が生徒のもので基準の時間以上
/// たったものを選んで経過の長い順に並べる（何を発言と数えるか・既定・範囲・境界は domain だけ）。各スレッドには、
/// 担当コーチと生徒の表示名（[`ThreadCoachDisplayNameLookup`]・[`ThreadStudentDisplayNameLookup`]）を引いて添える。
/// 誰が呼べるか（本部だけ）と、本文を返さないことは境界（interface）が守る。
pub struct ListUnansweredMessageThreads<R, D, S, C> {
    threads: R,
    coach_names: D,
    student_names: S,
    clock: C,
}

impl<R, D, S, C> ListUnansweredMessageThreads<R, D, S, C>
where
    R: MessageThreadRepository,
    D: ThreadCoachDisplayNameLookup,
    S: ThreadStudentDisplayNameLookup,
    C: Clock,
{
    pub fn new(threads: R, coach_names: D, student_names: S, clock: C) -> Self {
        Self {
            threads,
            coach_names,
            student_names,
            clock,
        }
    }

    pub async fn execute(
        &self,
        criteria: UnansweredThreadCriteria,
    ) -> Result<UnansweredThreadList, Unavailable> {
        let now = self.clock.now();
        let threads = criteria.select(self.threads.list_all().await?, now);
        if threads.is_empty() {
            return Ok(UnansweredThreadList {
                criteria,
                listed_at: now,
                threads,
                coach_display_names: BTreeMap::new(),
                student_display_names: BTreeMap::new(),
            });
        }

        let coach_ids: BTreeSet<AccountId> = threads.iter().map(MessageThread::coach_id).collect();
        let student_ids: BTreeSet<AccountId> =
            threads.iter().map(MessageThread::student_id).collect();
        let coach_display_names = self
            .coach_names
            .find_coach_display_names(&coach_ids)
            .await?;
        let student_display_names = self
            .student_names
            .find_student_display_names(&student_ids)
            .await?;
        Ok(UnansweredThreadList {
            criteria,
            listed_at: now,
            threads,
            coach_display_names,
            student_display_names,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, SystemTime},
    };

    use domain::{
        messaging::{MessageBody, MessageContent, MessageId, MessageThreadId},
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryMessageThreadRepository};

    const HOUR: Duration = Duration::from_secs(3600);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn now() -> SystemTime {
        // 2026-11-01T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    fn name(value: &str) -> DisplayName {
        DisplayName::new(value).unwrap()
    }

    /// `id` の `student` と `coach` のスレッドを照会の 10 日前に開き、`exchange` の順に（生徒が送ったか, 何時間前か）で
    /// 本文を送る。
    fn thread(
        id: u128,
        student: AccountId,
        coach: AccountId,
        exchange: &[(bool, u64)],
    ) -> MessageThread {
        let mut thread = MessageThread::open(
            MessageThreadId::from_u128(id),
            student,
            coach,
            now() - 240 * HOUR,
        )
        .unwrap();
        for (n, (by_student, hours)) in exchange.iter().enumerate() {
            thread
                .send(
                    MessageId::from_u128(id * 100 + n as u128),
                    if *by_student { student } else { coach },
                    MessageContent::Text(MessageBody::new("本部には見せない本文").unwrap()),
                    true,
                    now() - *hours as u32 * HOUR,
                )
                .unwrap();
        }
        thread
    }

    /// `COACH` と `STUDENT` の表示名だけを知っていて、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        coaches_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
        students_asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl ThreadCoachDisplayNameLookup for RecordingLookup {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.coaches_asked.lock().unwrap().push(coach_ids.clone());
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| (*id, name("たなか")))
                .collect())
        }
    }

    impl ThreadStudentDisplayNameLookup for RecordingLookup {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.students_asked
                .lock()
                .unwrap()
                .push(student_ids.clone());
            Ok(student_ids
                .iter()
                .filter(|id| **id == STUDENT)
                .map(|id| (*id, name("あおき")))
                .collect())
        }
    }

    fn use_case(
        threads: InMemoryMessageThreadRepository,
        lookup: RecordingLookup,
    ) -> ListUnansweredMessageThreads<
        InMemoryMessageThreadRepository,
        RecordingLookup,
        RecordingLookup,
        FixedClock,
    > {
        ListUnansweredMessageThreads::new(threads, lookup.clone(), lookup, FixedClock(now()))
    }

    fn threads() -> InMemoryMessageThreadRepository {
        InMemoryMessageThreadRepository::with_threads([
            // 24 時間ちょうど前の生徒の発言
            thread(1, STUDENT, COACH, &[(true, 24)]),
            // コーチが返した
            thread(2, OTHER_STUDENT, COACH, &[(true, 90), (false, 80)]),
            // 23 時間前の生徒の発言
            thread(3, STUDENT, OTHER_COACH, &[(true, 23)]),
            // アカウントのないコーチ・生徒の、50 時間前の生徒の発言
            thread(
                4,
                AccountId::from_u128(4),
                AccountId::from_u128(99),
                &[(true, 50)],
            ),
            // 発言がまだない
            thread(5, OTHER_STUDENT, OTHER_COACH, &[]),
        ])
    }

    fn ids(list: &UnansweredThreadList) -> Vec<u128> {
        list.threads.iter().map(|t| t.id().as_u128()).collect()
    }

    #[tokio::test]
    async fn lists_threads_waiting_for_the_coach_for_twenty_four_hours_or_more_longest_first() {
        let lookup = RecordingLookup::default();
        let list = use_case(threads(), lookup.clone())
            .execute(UnansweredThreadCriteria::default())
            .await
            .unwrap();
        assert_eq!(list.criteria.hours(), 24);
        assert_eq!(list.listed_at, now());
        assert_eq!(ids(&list), [4, 1]);
        assert_eq!(
            UnansweredThreadCriteria::elapsed(&list.threads[0], list.listed_at),
            Some(50 * HOUR)
        );
        assert_eq!(
            list.coach_display_names,
            BTreeMap::from([(COACH, name("たなか"))])
        );
        assert_eq!(
            list.student_display_names,
            BTreeMap::from([(STUDENT, name("あおき"))])
        );
        assert_eq!(
            *lookup.coaches_asked.lock().unwrap(),
            [BTreeSet::from([COACH, AccountId::from_u128(99)])],
            "一覧のコーチを重ねずに 1 回で問い合わせる"
        );
        assert_eq!(
            *lookup.students_asked.lock().unwrap(),
            [BTreeSet::from([STUDENT, AccountId::from_u128(4)])]
        );
    }

    #[tokio::test]
    async fn the_hours_of_the_criteria_change_the_threshold() {
        for (hours, expected) in [(1, vec![4, 1, 3]), (25, vec![4]), (168, vec![])] {
            let list = use_case(threads(), RecordingLookup::default())
                .execute(UnansweredThreadCriteria::new(hours).unwrap())
                .await
                .unwrap();
            assert_eq!(list.criteria.hours(), hours);
            assert_eq!(ids(&list), expected, "{hours} 時間");
        }
    }

    #[tokio::test]
    async fn no_unanswered_threads_is_an_empty_list_without_asking_for_display_names() {
        let lookup = RecordingLookup::default();
        let list = use_case(
            InMemoryMessageThreadRepository::with_threads([
                thread(1, STUDENT, COACH, &[(true, 2)]),
                thread(2, STUDENT, OTHER_COACH, &[(true, 30), (false, 29)]),
            ]),
            lookup.clone(),
        )
        .execute(UnansweredThreadCriteria::default())
        .await
        .unwrap();
        assert!(list.threads.is_empty());
        assert!(list.coach_display_names.is_empty() && list.student_display_names.is_empty());
        assert!(lookup.coaches_asked.lock().unwrap().is_empty());
        assert!(lookup.students_asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn listing_does_not_mark_messages_read() {
        let repository = threads();
        let before = repository.all();
        use_case(repository.clone(), RecordingLookup::default())
            .execute(UnansweredThreadCriteria::default())
            .await
            .unwrap();
        assert_eq!(repository.all(), before, "既読にしない");
    }
}
