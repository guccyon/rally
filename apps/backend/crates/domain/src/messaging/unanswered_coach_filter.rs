use super::CoachUnansweredMessageThreadCount;

/// 本部がコーチを「返事のないメッセージのあるコーチ」に絞るときの条件（admin-web の `listCoaches` の
/// `has_unanswered`）。「返事のないメッセージがある」をどう決めるかはここだけに置く。
///
/// - `has_unanswered` が `true`: 返信のないまま既定の時間の過ぎたスレッドの数
///   （[`CoachUnansweredMessageThreadCount::unanswered_count`]。`getCoachOverview` の
///   `unanswered_message_thread_count` と同じ数で、`listUnansweredMessageThreads` に既定の条件とそのコーチの `coach_id`
///   だけを付けたときに載るスレッドの数）が 1 件以上のコーチだけ
/// - `false`（既定）: 絞らない（どのコーチも通す）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnansweredCoachFilter(bool);

impl UnansweredCoachFilter {
    /// `has_unanswered` が `true` なら返事のないメッセージのあるコーチだけに絞る条件。`false` なら絞らない。
    pub const fn new(has_unanswered: bool) -> Self {
        Self(has_unanswered)
    }

    /// 返事のないメッセージのあるコーチだけに絞るか。
    pub const fn has_unanswered(&self) -> bool {
        self.0
    }

    /// 照会した時刻で数えた `unanswered_threads` のコーチがこの条件に合うか。
    pub const fn matches(&self, unanswered_threads: &CoachUnansweredMessageThreadCount) -> bool {
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
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn now() -> SystemTime {
        // 2026-11-01T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    /// `id` の生徒と `coach` のスレッドを照会の 10 日前に開き、`exchange` の順に（生徒が送ったか, 送信時刻）で
    /// 本文を送る。
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

    fn tally(threads: &[MessageThread]) -> CoachUnansweredMessageThreadCount {
        CoachUnansweredMessageThreadCount::tally(COACH, threads, now())
    }

    #[test]
    fn has_unanswered_matches_only_coaches_with_an_unanswered_thread() {
        let filter = UnansweredCoachFilter::new(true);
        assert!(filter.has_unanswered());
        assert!(!filter.matches(&tally(&[])), "スレッドがなければ載らない");
        assert!(
            filter.matches(&tally(&[
                thread(
                    1,
                    COACH,
                    &[(true, now() - 50 * HOUR), (false, now() - HOUR)]
                ),
                // 24 時間ちょうど前の生徒の発言は数える
                thread(2, COACH, &[(true, now() - 24 * HOUR)]),
            ])),
            "返信のないまま既定の時間の過ぎたスレッドが 1 件でもあれば載る"
        );
        assert!(
            !filter.matches(&tally(&[
                // コーチが返した・24 時間に 1 秒足りない・発言がない・他のコーチのスレッドは数えない
                thread(
                    1,
                    COACH,
                    &[(true, now() - 50 * HOUR), (false, now() - HOUR)]
                ),
                thread(2, COACH, &[(true, now() - 24 * HOUR + SECOND)]),
                thread(3, COACH, &[]),
                thread(4, OTHER_COACH, &[(true, now() - 72 * HOUR)]),
            ])),
            "数えないスレッドだけなら載らない"
        );
    }

    #[test]
    fn has_unanswered_is_the_same_as_the_default_list_of_the_coach_being_non_empty() {
        let threads = vec![
            thread(1, COACH, &[(true, now() - 24 * HOUR)]),
            thread(2, OTHER_COACH, &[(true, now() - 72 * HOUR), (false, now())]),
        ];
        let filter = UnansweredCoachFilter::new(true);
        for coach_id in [COACH, OTHER_COACH, STUDENT] {
            for at in [now(), now() - SECOND, now() + 100 * HOUR] {
                let listed = UnansweredThreadCriteria::default()
                    .with_coach_id(Some(coach_id))
                    .select(threads.clone(), at);
                let count = CoachUnansweredMessageThreadCount::tally(coach_id, &threads, at);
                assert_eq!(
                    filter.matches(&count),
                    !listed.is_empty(),
                    "{coach_id:?} {at:?}"
                );
            }
        }
    }

    #[test]
    fn not_has_unanswered_matches_every_coach() {
        let filter = UnansweredCoachFilter::default();
        assert_eq!(filter, UnansweredCoachFilter::new(false));
        assert!(!filter.has_unanswered());
        assert!(filter.matches(&tally(&[])));
        assert!(filter.matches(&tally(&[thread(1, COACH, &[(true, now() - 24 * HOUR)])])));
    }
}
