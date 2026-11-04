use std::time::{Duration, SystemTime};

use super::{MessageThread, MessagingError};
use crate::identity::AccountId;

/// 本部が見る「返信のないまま時間の過ぎた」スレッドの基準（admin-web の `listUnansweredMessageThreads`。
/// 既定・範囲・境界の正本は `specs/contracts/api/admin-web/openapi.yaml` の `listUnansweredMessageThreads`）。
///
/// 最後の発言が生徒のもので、照会した時刻に返信を待ち始めてから（コーチの最後の発言のあとの最初の生徒の発言から。
/// [`MessageThread::awaiting_coach_reply_since`]）[`Self::hours`] 時間以上たったスレッドを「返信のないまま時間の過ぎた」スレッドとする。時間は
/// [`Self::MIN_HOURS`]〜[`Self::MAX_HOURS`] 時間で、省いたときは [`Self::DEFAULT_HOURS`] 時間（[`Default`]）。
/// 既定・範囲と並べ方はこの型の 1 か所だけに置く。任意の担当コーチ [`Self::coach_id`]（[`Self::with_coach_id`]。
/// 省けばすべてのコーチ）と生徒 [`Self::student_id`]（[`Self::with_student_id`]。省けばすべての生徒）で絞れ、
/// 時間と組み合わせても時間の規則と並べ方は変わらない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnansweredThreadCriteria {
    hours: u16,
    coach_id: Option<AccountId>,
    student_id: Option<AccountId>,
}

impl UnansweredThreadCriteria {
    /// 時間を省いたときの、返信を待ち始めてからの時間（24 時間）
    pub const DEFAULT_HOURS: u16 = 24;
    /// 指定できる時間の下限（1 時間）
    pub const MIN_HOURS: u16 = 1;
    /// 指定できる時間の上限（7 日 = 168 時間）
    pub const MAX_HOURS: u16 = 168;

    /// 返信を待ち始めてから `hours` 時間以上たったスレッドを見る基準。[`Self::MIN_HOURS`]〜[`Self::MAX_HOURS`] の外なら
    /// [`MessagingError::UnansweredThreadHoursOutOfRange`]。
    pub fn new(hours: u16) -> Result<Self, MessagingError> {
        if !(Self::MIN_HOURS..=Self::MAX_HOURS).contains(&hours) {
            return Err(MessagingError::UnansweredThreadHoursOutOfRange {
                min: Self::MIN_HOURS,
                max: Self::MAX_HOURS,
            });
        }
        Ok(Self {
            hours,
            coach_id: None,
            student_id: None,
        })
    }

    /// `coach_id` があればそのコーチが担当するスレッドだけを選ぶ基準（`None` ならすべてのコーチ）。時間は変えない。
    pub const fn with_coach_id(self, coach_id: Option<AccountId>) -> Self {
        Self { coach_id, ..self }
    }

    /// `student_id` があればその生徒のスレッドだけを選ぶ基準（`None` ならすべての生徒）。時間と担当コーチは変えない。
    pub const fn with_student_id(self, student_id: Option<AccountId>) -> Self {
        Self { student_id, ..self }
    }

    /// 返信を待ち始めてからの時間（時間単位）。
    pub const fn hours(&self) -> u16 {
        self.hours
    }

    /// 絞り込む担当コーチ。`None` ならすべてのコーチ。
    pub const fn coach_id(&self) -> Option<AccountId> {
        self.coach_id
    }

    /// 絞り込む生徒。`None` ならすべての生徒。
    pub const fn student_id(&self) -> Option<AccountId> {
        self.student_id
    }

    /// 返信を待ち始めてからの時間の長さ。
    pub const fn threshold(&self) -> Duration {
        Duration::from_secs(self.hours as u64 * 3600)
    }

    /// `thread` が `now` に照会したときに返信を待っているなら、返信を待ち始めてから `now` までの経過時間。
    /// 待っていなければ `None`。待ち始めが `now` より後（時計のずれ）なら 0。
    pub fn elapsed(thread: &MessageThread, now: SystemTime) -> Option<Duration> {
        thread
            .awaiting_coach_reply_since()
            .map(|since| now.duration_since(since).unwrap_or_default())
    }

    /// `thread` が `now` に照会したときの、返信のないまま時間の過ぎたスレッド（ちょうどその時間を含む）で、コーチの指定が
    /// あればそのコーチが担当し、生徒の指定があればその生徒のスレッドか。
    pub fn matches(&self, thread: &MessageThread, now: SystemTime) -> bool {
        self.coach_id
            .is_none_or(|coach_id| thread.coach_id() == coach_id)
            && self
                .student_id
                .is_none_or(|student_id| thread.student_id() == student_id)
            && Self::elapsed(thread, now).is_some_and(|elapsed| elapsed >= self.threshold())
    }

    /// `threads` から `now` に照会したときの返信のないまま時間の過ぎたスレッドだけを選び、経過の長い順（待ち始めの
    /// 古い順。同じ時刻はスレッドの ID の順）に並べる。
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
            coach_id: None,
            student_id: None,
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
        assert_eq!(criteria.coach_id(), None, "既定はすべてのコーチ");
        assert_eq!(criteria.student_id(), None, "既定はすべての生徒");
    }

    #[test]
    fn the_student_narrows_to_the_threads_of_the_student() {
        let waiting = thread(1, &[(STUDENT, now() - 30 * HOUR)]);
        let criteria = UnansweredThreadCriteria::new(24)
            .unwrap()
            .with_student_id(Some(STUDENT));
        assert_eq!(criteria.hours(), 24, "時間は変えない");
        assert_eq!(criteria.student_id(), Some(STUDENT));
        assert_eq!(criteria.coach_id(), None, "コーチは変えない");
        assert!(criteria.matches(&waiting, now()));
        // ほかの生徒・担当コーチの ID では出ない
        for other in [AccountId::from_u128(8), COACH] {
            assert!(
                !UnansweredThreadCriteria::default()
                    .with_student_id(Some(other))
                    .matches(&waiting, now()),
                "{other:?}"
            );
        }
        // 時間の規則は変わらない
        assert!(
            !UnansweredThreadCriteria::new(31)
                .unwrap()
                .with_student_id(Some(STUDENT))
                .matches(&waiting, now())
        );
        // コーチと組み合わせれば両方に合うものだけ
        assert!(criteria.with_coach_id(Some(COACH)).matches(&waiting, now()));
        assert!(
            !criteria
                .with_coach_id(Some(AccountId::from_u128(8)))
                .matches(&waiting, now())
        );
        assert_eq!(
            criteria.with_coach_id(Some(COACH)).student_id(),
            Some(STUDENT),
            "コーチを足しても生徒は変えない"
        );
        // 指定を外せばすべての生徒
        assert_eq!(
            criteria.with_student_id(None),
            UnansweredThreadCriteria::default()
        );
    }

    #[test]
    fn the_coach_narrows_to_the_threads_the_coach_is_in_charge_of() {
        let waiting = thread(1, &[(STUDENT, now() - 30 * HOUR)]);
        let criteria = UnansweredThreadCriteria::new(24)
            .unwrap()
            .with_coach_id(Some(COACH));
        assert_eq!(criteria.hours(), 24, "時間は変えない");
        assert_eq!(criteria.coach_id(), Some(COACH));
        assert!(criteria.matches(&waiting, now()));
        // ほかのコーチ・当事者の生徒の ID では出ない
        for other in [AccountId::from_u128(8), STUDENT] {
            assert!(
                !UnansweredThreadCriteria::default()
                    .with_coach_id(Some(other))
                    .matches(&waiting, now()),
                "{other:?}"
            );
        }
        // 時間の規則は変わらない
        assert!(
            !UnansweredThreadCriteria::new(31)
                .unwrap()
                .with_coach_id(Some(COACH))
                .matches(&waiting, now())
        );
        // 指定を外せばすべてのコーチ
        assert_eq!(
            criteria.with_coach_id(None),
            UnansweredThreadCriteria::default()
        );
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
    fn a_reminder_from_the_student_does_not_reset_the_elapsed_time() {
        // 30 時間前に送り、2 時間前に催促した
        let reminded = thread(
            1,
            &[(STUDENT, now() - 30 * HOUR), (STUDENT, now() - 2 * HOUR)],
        );
        assert_eq!(
            UnansweredThreadCriteria::elapsed(&reminded, now()),
            Some(30 * HOUR)
        );
        assert!(UnansweredThreadCriteria::default().matches(&reminded, now()));
        // コーチが返したあとの発言からは数え直す
        let answered = thread(
            2,
            &[
                (STUDENT, now() - 30 * HOUR),
                (COACH, now() - 20 * HOUR),
                (STUDENT, now() - 10 * HOUR),
                (STUDENT, now() - HOUR),
            ],
        );
        assert_eq!(
            UnansweredThreadCriteria::elapsed(&answered, now()),
            Some(10 * HOUR)
        );
        assert!(!UnansweredThreadCriteria::default().matches(&answered, now()));
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
