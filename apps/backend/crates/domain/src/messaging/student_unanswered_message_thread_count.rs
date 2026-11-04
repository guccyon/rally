use std::time::SystemTime;

use super::{MessageThread, UnansweredThreadCriteria};
use crate::identity::AccountId;

/// 生徒 1 人の、返信のないまま時間の過ぎたスレッドの数（admin-web の `getStudentActivity` の
/// `unanswered_message_thread_count`。正本は `specs/contracts/api/admin-web/openapi.yaml` の `getStudentActivity`）。
///
/// `listUnansweredMessageThreads` に既定の条件（時間を省いた [`UnansweredThreadCriteria::default`]）とその生徒の
/// `student_id` だけを付けたときに載るスレッドの数と同じ。何を発言と数えるか・既定の時間・境界は
/// [`UnansweredThreadCriteria`] の 1 か所だけで、ここでは書き直さない（[`Self::criteria`] の基準の
/// [`UnansweredThreadCriteria::matches`] で数える）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudentUnansweredMessageThreadCount {
    student_id: AccountId,
    unanswered_count: u32,
}

impl StudentUnansweredMessageThreadCount {
    /// 生徒 `student_id` の数を数える基準（既定の時間で、その生徒のスレッドだけ。担当コーチでは絞らない）。
    pub fn criteria(student_id: AccountId) -> UnansweredThreadCriteria {
        UnansweredThreadCriteria::default().with_student_id(Some(student_id))
    }

    /// `threads` のうち、`now` に照会したときに生徒 `student_id` の返信のないまま既定の時間の過ぎたスレッドを数える（他の
    /// 生徒のスレッド・コーチが最後に発言したスレッド・発言のないスレッド・既定の時間に満たないスレッドは数えない）。
    pub fn tally<'a>(
        student_id: AccountId,
        threads: impl IntoIterator<Item = &'a MessageThread>,
        now: SystemTime,
    ) -> Self {
        let criteria = Self::criteria(student_id);
        let n = threads
            .into_iter()
            .filter(|thread| criteria.matches(thread, now))
            .count();
        Self {
            student_id,
            unanswered_count: u32::try_from(n).unwrap_or(u32::MAX),
        }
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    /// 返信のないまま既定の時間の過ぎたスレッドの数。
    pub const fn unanswered_count(&self) -> u32 {
        self.unanswered_count
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::messaging::{MessageBody, MessageContent, MessageId, MessageThreadId};

    const HOUR: Duration = Duration::from_secs(3600);
    const SECOND: Duration = Duration::from_secs(1);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
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
                    MessageContent::Text(MessageBody::new("質問です").unwrap()),
                    true,
                    *at,
                )
                .unwrap();
        }
        thread
    }

    fn threads() -> Vec<MessageThread> {
        vec![
            // 24 時間ちょうど前の生徒の発言（含む）
            thread(1, STUDENT, COACH, &[(true, now() - 24 * HOUR)]),
            // 別のコーチとのスレッドで、24 時間に 1 秒足りない
            thread(
                2,
                STUDENT,
                OTHER_COACH,
                &[(true, now() - 24 * HOUR + SECOND)],
            ),
            // コーチが返した
            thread(
                3,
                STUDENT,
                AccountId::from_u128(9),
                &[(true, now() - 50 * HOUR), (false, now() - HOUR)],
            ),
            // 発言がまだない
            thread(4, STUDENT, AccountId::from_u128(10), &[]),
            // 30 時間前に送り、1 時間前に催促した（待ち始めは 30 時間前のまま）
            thread(
                5,
                STUDENT,
                AccountId::from_u128(11),
                &[(true, now() - 30 * HOUR), (true, now() - HOUR)],
            ),
            // 他の生徒
            thread(6, OTHER_STUDENT, COACH, &[(true, now() - 72 * HOUR)]),
        ]
    }

    #[test]
    fn the_criteria_is_the_default_narrowed_to_the_student() {
        let criteria = StudentUnansweredMessageThreadCount::criteria(STUDENT);
        assert_eq!(
            criteria,
            UnansweredThreadCriteria::default().with_student_id(Some(STUDENT))
        );
        assert_eq!(criteria.hours(), UnansweredThreadCriteria::DEFAULT_HOURS);
        assert_eq!(criteria.coach_id(), None, "コーチでは絞らない");
    }

    #[test]
    fn tallies_unanswered_threads_of_the_student_with_the_default_hours() {
        let count = StudentUnansweredMessageThreadCount::tally(STUDENT, &threads(), now());
        assert_eq!(count.student_id(), STUDENT);
        assert_eq!(
            count.unanswered_count(),
            2,
            "24 時間ちょうどと催促したスレッドを数え、時間に満たない・返した・発言のない・他の生徒のスレッドは数えない"
        );

        let other = StudentUnansweredMessageThreadCount::tally(OTHER_STUDENT, &threads(), now());
        assert_eq!(other.unanswered_count(), 1);

        // コーチの ID では当事者でも数えない（生徒として絞る）
        let coach = StudentUnansweredMessageThreadCount::tally(COACH, &threads(), now());
        assert_eq!(coach.unanswered_count(), 0);

        let empty = StudentUnansweredMessageThreadCount::tally(STUDENT, &[], now());
        assert_eq!(empty.unanswered_count(), 0);
    }

    #[test]
    fn the_count_is_the_same_as_the_default_list_of_the_student() {
        for student_id in [STUDENT, OTHER_STUDENT, COACH, AccountId::from_u128(99)] {
            for at in [now(), now() + SECOND, now() - 23 * HOUR, now() + 100 * HOUR] {
                let listed = UnansweredThreadCriteria::default()
                    .with_student_id(Some(student_id))
                    .select(threads(), at)
                    .len();
                assert_eq!(
                    StudentUnansweredMessageThreadCount::tally(student_id, &threads(), at)
                        .unanswered_count() as usize,
                    listed,
                    "{student_id:?} {at:?}"
                );
            }
        }
    }
}
