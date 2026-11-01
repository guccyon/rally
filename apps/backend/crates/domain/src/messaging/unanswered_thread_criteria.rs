use std::time::{Duration, SystemTime};

use super::{MessageThread, MessagingError};

/// 本部が見る「返信のないまま時間の過ぎた」スレッドの基準（admin-web の `listUnansweredMessageThreads`。
/// 既定・範囲・境界の正本は `specs/contracts/api/admin-web/openapi.yaml` の `listUnansweredMessageThreads`）。
///
/// 最後の発言が生徒のもので（[`MessageThread::awaiting_coach_reply_since`]）、照会した時刻にその発言から
/// [`Self::hours`] 時間以上たったスレッドを「返信のないまま時間の過ぎた」スレッドとする。時間は
/// [`Self::MIN_HOURS`]〜[`Self::MAX_HOURS`] 時間で、省いたときは [`Self::DEFAULT_HOURS`] 時間（[`Default`]）。
/// 既定・範囲と並べ方はこの型の 1 か所だけに置く。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnansweredThreadCriteria {
    hours: u16,
}

impl UnansweredThreadCriteria {
    /// 時間を省いたときの、生徒の最後の発言からの時間（24 時間）
    pub const DEFAULT_HOURS: u16 = 24;
    /// 指定できる時間の下限（1 時間）
    pub const MIN_HOURS: u16 = 1;
    /// 指定できる時間の上限（7 日 = 168 時間）
    pub const MAX_HOURS: u16 = 168;

    /// 生徒の最後の発言から `hours` 時間以上たったスレッドを見る基準。[`Self::MIN_HOURS`]〜[`Self::MAX_HOURS`] の外なら
    /// [`MessagingError::UnansweredThreadHoursOutOfRange`]。
    pub fn new(hours: u16) -> Result<Self, MessagingError> {
        if !(Self::MIN_HOURS..=Self::MAX_HOURS).contains(&hours) {
            return Err(MessagingError::UnansweredThreadHoursOutOfRange {
                min: Self::MIN_HOURS,
                max: Self::MAX_HOURS,
            });
        }
        Ok(Self { hours })
    }

    /// 生徒の最後の発言からの時間（時間単位）。
    pub const fn hours(&self) -> u16 {
        self.hours
    }

    /// 生徒の最後の発言からの時間の長さ。
    pub const fn threshold(&self) -> Duration {
        Duration::from_secs(self.hours as u64 * 3600)
    }

    /// `thread` が `now` に照会したときに返信を待っているなら、生徒の最後の発言から `now` までの経過時間。
    /// 待っていなければ `None`。発言が `now` より後（時計のずれ）なら 0。
    pub fn elapsed(thread: &MessageThread, now: SystemTime) -> Option<Duration> {
        thread
            .awaiting_coach_reply_since()
            .map(|since| now.duration_since(since).unwrap_or_default())
    }

    /// `thread` が `now` に照会したときの、返信のないまま時間の過ぎたスレッドか（ちょうどその時間を含む）。
    pub fn matches(&self, thread: &MessageThread, now: SystemTime) -> bool {
        Self::elapsed(thread, now).is_some_and(|elapsed| elapsed >= self.threshold())
    }

    /// `threads` から `now` に照会したときの返信のないまま時間の過ぎたスレッドだけを選び、経過の長い順（生徒の最後の
    /// 発言の古い順。同じ時刻はスレッドの ID の順）に並べる。
    pub fn select(&self, threads: Vec<MessageThread>, now: SystemTime) -> Vec<MessageThread> {
        let mut unanswered: Vec<MessageThread> = threads
            .into_iter()
            .filter(|thread| self.matches(thread, now))
            .collect();
        unanswered.sort_by_key(|thread| (thread.awaiting_coach_reply_since(), thread.id()));
        unanswered
    }
}

impl Default for UnansweredThreadCriteria {
    fn default() -> Self {
        Self {
            hours: Self::DEFAULT_HOURS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::AccountId,
        messaging::{MessageBody, MessageContent, MessageId, MessageThreadId},
    };

    const HOUR: Duration = Duration::from_secs(3600);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(2);

    fn now() -> SystemTime {
        // 2026-11-01T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    fn text() -> MessageContent {
        MessageContent::Text(MessageBody::new("質問です").unwrap())
    }

    /// `id` のスレッドを照会の 10 日前に開き、`exchange` の順に（当事者のアカウント, 送信時刻）で本文を送る。
    fn thread(id: u128, exchange: &[(AccountId, SystemTime)]) -> MessageThread {
        let mut thread = MessageThread::open(
            MessageThreadId::from_u128(id),
            STUDENT,
            COACH,
            now() - 240 * HOUR,
        )
        .unwrap();
        for (n, (sender, at)) in exchange.iter().enumerate() {
            thread
                .send(
                    MessageId::from_u128(id * 100 + n as u128),
                    *sender,
                    text(),
                    true,
                    *at,
                )
                .unwrap();
        }
        thread
    }

    fn ids(threads: &[MessageThread]) -> Vec<u128> {
        threads.iter().map(|t| t.id().as_u128()).collect()
    }

    #[test]
    fn the_default_is_twenty_four_hours() {
        let criteria = UnansweredThreadCriteria::default();
        assert_eq!(criteria.hours(), 24);
        assert_eq!(criteria.threshold(), 24 * HOUR);
    }

    #[test]
    fn accepts_one_to_one_hundred_sixty_eight_hours() {
        for hours in [1, 24, 168] {
            assert_eq!(UnansweredThreadCriteria::new(hours).unwrap().hours(), hours);
        }
        for hours in [0, 169, u16::MAX] {
            assert_eq!(
                UnansweredThreadCriteria::new(hours),
                Err(MessagingError::UnansweredThreadHoursOutOfRange { min: 1, max: 168 }),
                "{hours}"
            );
        }
    }

    #[test]
    fn threads_whose_last_student_message_is_at_least_the_hours_old_match() {
        let criteria = UnansweredThreadCriteria::default();
        // ちょうど 24 時間前は含む。24 時間に 1 秒足りないものは含まない
        assert!(criteria.matches(&thread(1, &[(STUDENT, now() - 24 * HOUR)]), now()));
        assert!(!criteria.matches(
            &thread(1, &[(STUDENT, now() - 24 * HOUR + Duration::from_secs(1))]),
            now()
        ));
        // コーチが返したもの・発言のないものは、どれだけ前でも含まない
        assert!(!criteria.matches(
            &thread(
                1,
                &[(STUDENT, now() - 100 * HOUR), (COACH, now() - 99 * HOUR)]
            ),
            now()
        ));
        assert!(!criteria.matches(&thread(1, &[]), now()));
        assert_eq!(
            UnansweredThreadCriteria::elapsed(&thread(1, &[(STUDENT, now() - 30 * HOUR)]), now()),
            Some(30 * HOUR)
        );
        // 発言が照会より後（時計のずれ）は 0
        assert_eq!(
            UnansweredThreadCriteria::elapsed(&thread(1, &[(STUDENT, now() + HOUR)]), now()),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn the_hours_change_the_threshold() {
        let waiting = thread(1, &[(STUDENT, now() - 2 * HOUR)]);
        assert!(
            UnansweredThreadCriteria::new(1)
                .unwrap()
                .matches(&waiting, now())
        );
        assert!(
            !UnansweredThreadCriteria::new(3)
                .unwrap()
                .matches(&waiting, now())
        );
    }

    #[test]
    fn selects_unanswered_threads_longest_waiting_first() {
        let threads = vec![
            thread(1, &[(STUDENT, now() - 30 * HOUR)]),
            thread(
                2,
                &[(STUDENT, now() - 90 * HOUR), (COACH, now() - 80 * HOUR)],
            ),
            thread(3, &[(STUDENT, now() - HOUR)]),
            // コーチの返信の後に生徒がまた送った
            thread(
                6,
                &[
                    (STUDENT, now() - 100 * HOUR),
                    (COACH, now() - 90 * HOUR),
                    (STUDENT, now() - 50 * HOUR),
                ],
            ),
            thread(5, &[(STUDENT, now() - 48 * HOUR)]),
            thread(4, &[(STUDENT, now() - 48 * HOUR)]),
        ];
        assert_eq!(
            ids(&UnansweredThreadCriteria::default().select(threads, now())),
            [6, 4, 5, 1],
            "同じ時刻はスレッドの ID の順"
        );
    }
}
