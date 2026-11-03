use std::time::SystemTime;

use super::{MessageThread, UnansweredThreadCriteria};
use crate::identity::AccountId;

/// コーチ 1 人の、返信のないまま時間の過ぎたスレッドの数（admin-web の `getCoachOverview` の
/// `unanswered_message_thread_count`。正本は `specs/contracts/api/admin-web/openapi.yaml` の `getCoachOverview`）。
///
/// `listUnansweredMessageThreads` に既定の条件（時間を省いた [`UnansweredThreadCriteria::default`]）とそのコーチの
/// `coach_id` だけを付けたときに載るスレッドの数と同じ。何を発言と数えるか・既定の時間・境界は
/// [`UnansweredThreadCriteria`] の 1 か所だけで、ここでは書き直さない（[`Self::criteria`] の基準の
/// [`UnansweredThreadCriteria::matches`] で数える）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachUnansweredMessageThreadCount {
    coach_id: AccountId,
    unanswered_count: u32,
}

impl CoachUnansweredMessageThreadCount {
    /// コーチ `coach_id` の数を数える基準（既定の時間で、そのコーチが担当するスレッドだけ）。
    pub fn criteria(coach_id: AccountId) -> UnansweredThreadCriteria {
        UnansweredThreadCriteria::default().with_coach_id(Some(coach_id))
    }

    /// `threads` のうち、`now` に照会したときにコーチ `coach_id` の返信のないまま既定の時間の過ぎたスレッドを数える（他の
    /// コーチのスレッド・コーチが最後に発言したスレッド・発言のないスレッド・既定の時間に満たないスレッドは数えない）。
    pub fn tally<'a>(
        coach_id: AccountId,
        threads: impl IntoIterator<Item = &'a MessageThread>,
        now: SystemTime,
    ) -> Self {
        let criteria = Self::criteria(coach_id);
        let n = threads
            .into_iter()
            .filter(|thread| criteria.matches(thread, now))
            .count();
        Self {
            coach_id,
            unanswered_count: u32::try_from(n).unwrap_or(u32::MAX),
        }
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
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
    const COACH: AccountId = AccountId::from_u128(2);
    const OTHER_COACH: AccountId = AccountId::from_u128(3);

    fn now() -> SystemTime {
        // 2026-11-01T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    /// `id` の `coach` のスレッドを照会の 10 日前に開き、`exchange` の順に（生徒が送ったか, 送信時刻）で本文を送る。
    fn thread(id: u128, coach: AccountId, exchange: &[(bool, SystemTime)]) -> MessageThread {
        let mut thread = MessageThread::open(
            MessageThreadId::from_u128(id),
            STUDENT,
            coach,
            now() - 240 * HOUR,
        )
        .unwrap();
        for (n, (by_student, at)) in exchange.iter().enumerate() {
            thread
                .send(
                    MessageId::from_u128(id * 100 + n as u128),
                    if *by_student { STUDENT } else { coach },
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
            thread(1, COACH, &[(true, now() - 24 * HOUR)]),
            // 24 時間に 1 秒足りない
            thread(2, COACH, &[(true, now() - 24 * HOUR + SECOND)]),
            // コーチが返した
            thread(
                3,
                COACH,
                &[(true, now() - 50 * HOUR), (false, now() - HOUR)],
            ),
            // 発言がまだない
            thread(4, COACH, &[]),
            // 30 時間前に送り、1 時間前に催促した（待ち始めは 30 時間前のまま）
            thread(5, COACH, &[(true, now() - 30 * HOUR), (true, now() - HOUR)]),
            // 他のコーチ
            thread(6, OTHER_COACH, &[(true, now() - 72 * HOUR)]),
        ]
    }

    #[test]
    fn the_criteria_is_the_default_narrowed_to_the_coach() {
        let criteria = CoachUnansweredMessageThreadCount::criteria(COACH);
        assert_eq!(
            criteria,
            UnansweredThreadCriteria::default().with_coach_id(Some(COACH))
        );
        assert_eq!(criteria.hours(), UnansweredThreadCriteria::DEFAULT_HOURS);
        assert_eq!(criteria.student_id(), None, "生徒では絞らない");
    }

    #[test]
    fn tallies_unanswered_threads_of_the_coach_with_the_default_hours() {
        let count = CoachUnansweredMessageThreadCount::tally(COACH, &threads(), now());
        assert_eq!(count.coach_id(), COACH);
        assert_eq!(
            count.unanswered_count(),
            2,
            "24 時間ちょうどと催促したスレッドを数え、時間に満たない・返した・発言のない・他のコーチのスレッドは数えない"
        );

        let other = CoachUnansweredMessageThreadCount::tally(OTHER_COACH, &threads(), now());
        assert_eq!(other.unanswered_count(), 1);

        let empty = CoachUnansweredMessageThreadCount::tally(COACH, &[], now());
        assert_eq!(empty.unanswered_count(), 0);
    }

    #[test]
    fn the_count_is_the_same_as_the_default_list_of_the_coach() {
        for coach_id in [COACH, OTHER_COACH, AccountId::from_u128(99)] {
            for at in [now(), now() - 23 * HOUR, now() + 100 * HOUR] {
                let listed = UnansweredThreadCriteria::default()
                    .with_coach_id(Some(coach_id))
                    .select(threads(), at)
                    .len();
                assert_eq!(
                    CoachUnansweredMessageThreadCount::tally(coach_id, &threads(), at)
                        .unanswered_count() as usize,
                    listed,
                    "{coach_id:?} {at:?}"
                );
            }
        }
    }
}
