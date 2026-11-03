use super::StudentUnansweredMessageThreadCount;

/// 本部が生徒を「返事を待っているメッセージのある生徒」に絞るときの条件（admin-web の `listStudents` の
/// `has_unanswered`）。「返事を待っているメッセージがある」をどう決めるかはここだけに置く。
///
/// - `has_unanswered` が `true`: 返信のないまま既定の時間の過ぎたスレッドの数
///   （[`StudentUnansweredMessageThreadCount::unanswered_count`]。`getStudentActivity` の
///   `unanswered_message_thread_count` と同じ数で、`listUnansweredMessageThreads` に既定の条件とその生徒の `student_id`
///   だけを付けたときに載るスレッドの数）が 1 件以上の生徒だけ
/// - `false`（既定）: 絞らない（どの生徒も通す）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnansweredStudentFilter(bool);

impl UnansweredStudentFilter {
    /// `has_unanswered` が `true` なら返事を待っているメッセージのある生徒だけに絞る条件。`false` なら絞らない。
    pub const fn new(has_unanswered: bool) -> Self {
        Self(has_unanswered)
    }

    /// 返事を待っているメッセージのある生徒だけに絞るか。
    pub const fn has_unanswered(&self) -> bool {
        self.0
    }

    /// 照会した時刻で数えた `unanswered_threads` の生徒がこの条件に合うか。
    pub const fn matches(&self, unanswered_threads: &StudentUnansweredMessageThreadCount) -> bool {
        !self.0 || unanswered_threads.unanswered_count() > 0
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::{
        identity::AccountId,
        messaging::{
            MessageBody, MessageContent, MessageId, MessageThread, MessageThreadId,
            UnansweredThreadCriteria,
        },
    };

    const HOUR: Duration = Duration::from_secs(3600);
    const SECOND: Duration = Duration::from_secs(1);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);

    fn now() -> SystemTime {
        // 2026-11-01T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    /// `id` の `student` と担当コーチのスレッドを照会の 10 日前に開き、`exchange` の順に（生徒が送ったか, 送信時刻）で
    /// 本文を送る。
    fn thread(id: u128, student: AccountId, exchange: &[(bool, SystemTime)]) -> MessageThread {
        let mut thread = MessageThread::open(
            MessageThreadId::from_u128(id),
            student,
            COACH,
            now() - 240 * HOUR,
        )
        .unwrap();
        for (n, (by_student, at)) in exchange.iter().enumerate() {
            thread
                .send(
                    MessageId::from_u128(id * 100 + n as u128),
                    if *by_student { student } else { COACH },
                    MessageContent::Text(MessageBody::new("質問です").unwrap()),
                    true,
                    *at,
                )
                .unwrap();
        }
        thread
    }

    fn tally(threads: &[MessageThread]) -> StudentUnansweredMessageThreadCount {
        StudentUnansweredMessageThreadCount::tally(STUDENT, threads, now())
    }

    #[test]
    fn has_unanswered_matches_only_students_with_an_unanswered_thread() {
        let filter = UnansweredStudentFilter::new(true);
        assert!(filter.has_unanswered());
        assert!(!filter.matches(&tally(&[])), "スレッドがなければ載らない");
        assert!(
            filter.matches(&tally(&[
                thread(
                    1,
                    STUDENT,
                    &[(true, now() - 50 * HOUR), (false, now() - HOUR)]
                ),
                // 24 時間ちょうど前の生徒の発言は数える
                thread(2, STUDENT, &[(true, now() - 24 * HOUR)]),
            ])),
            "返信のないまま既定の時間の過ぎたスレッドが 1 件でもあれば載る"
        );
        assert!(
            !filter.matches(&tally(&[
                // コーチが返した・24 時間に 1 秒足りない・発言がない・他の生徒のスレッドは数えない
                thread(
                    1,
                    STUDENT,
                    &[(true, now() - 50 * HOUR), (false, now() - HOUR)]
                ),
                thread(2, STUDENT, &[(true, now() - 24 * HOUR + SECOND)]),
                thread(3, STUDENT, &[]),
                thread(4, OTHER_STUDENT, &[(true, now() - 72 * HOUR)]),
            ])),
            "数えないスレッドだけなら載らない"
        );
    }

    #[test]
    fn has_unanswered_is_the_same_as_the_default_list_of_the_student_being_non_empty() {
        let threads = vec![
            thread(1, STUDENT, &[(true, now() - 24 * HOUR)]),
            thread(
                2,
                OTHER_STUDENT,
                &[(true, now() - 72 * HOUR), (false, now())],
            ),
        ];
        let filter = UnansweredStudentFilter::new(true);
        for student_id in [STUDENT, OTHER_STUDENT, COACH] {
            for at in [now(), now() - SECOND, now() + 100 * HOUR] {
                let listed = UnansweredThreadCriteria::default()
                    .with_student_id(Some(student_id))
                    .select(threads.clone(), at);
                let count = StudentUnansweredMessageThreadCount::tally(student_id, &threads, at);
                assert_eq!(
                    filter.matches(&count),
                    !listed.is_empty(),
                    "{student_id:?} {at:?}"
                );
            }
        }
    }

    #[test]
    fn not_has_unanswered_matches_every_student() {
        let filter = UnansweredStudentFilter::default();
        assert_eq!(filter, UnansweredStudentFilter::new(false));
        assert!(!filter.has_unanswered());
        assert!(filter.matches(&tally(&[])));
        assert!(filter.matches(&tally(&[thread(1, STUDENT, &[(true, now() - 24 * HOUR)])])));
    }
}
