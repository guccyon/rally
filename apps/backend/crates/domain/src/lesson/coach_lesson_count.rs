use std::time::SystemTime;

use super::{LessonCompletion, LessonType, RecentLessonPeriod};
use crate::identity::AccountId;

/// コーチ 1 人の完了したレッスンの数を、同期レッスン（通話）と非同期レッスン（動画添削）に分けたもの
/// （admin-web の `getMyLessonStats`・`getCoachOverview` の月ごとの数と、`listCoaches` の直近 30 日の数）。
///
/// 数え方はこの型の 1 か所だけに置く。どの期間の記録を渡すか（暦月で区切る）は呼ぶ側が決め、直近の期間
/// （[`RecentLessonPeriod`]）で数えるときは [`Self::tally_recent`] を使う。
/// グループレッスンの通話セッションは、完了したレッスン（結び付いた予約の生徒ごとに 1 つ）の数だけ数える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoachLessonCount {
    coach_id: AccountId,
    live_count: u32,
    async_count: u32,
}

impl CoachLessonCount {
    /// `completions` のうち、コーチ `coach_id` の完了したレッスンを種類ごとに数える（他のコーチの記録は数えない）。
    pub fn tally(coach_id: AccountId, completions: &[LessonCompletion]) -> Self {
        Self::tally_where(coach_id, completions, |_| true)
    }

    /// `completions` のうち、コーチ `coach_id` の完了したレッスンで、`now` に照会したときの直近の期間
    /// （[`RecentLessonPeriod::contains`]）に完了したものを [`Self::tally`] と同じ数え方で数える。
    pub fn tally_recent(
        coach_id: AccountId,
        completions: &[LessonCompletion],
        now: SystemTime,
    ) -> Self {
        Self::tally_where(coach_id, completions, |completion| {
            RecentLessonPeriod::contains(completion, now)
        })
    }

    fn tally_where(
        coach_id: AccountId,
        completions: &[LessonCompletion],
        in_period: impl Fn(&LessonCompletion) -> bool,
    ) -> Self {
        let (live_count, async_count) = completions
            .iter()
            .filter(|completion| completion.coach_id() == coach_id && in_period(completion))
            .fold(
                (0u32, 0u32),
                |(live, async_), completion| match completion.lesson_type() {
                    LessonType::Live => (live + 1, async_),
                    LessonType::Async => (live, async_ + 1),
                },
            );
        Self {
            coach_id,
            live_count,
            async_count,
        }
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// 同期レッスン（通話）の完了したレッスンの数。
    pub const fn live_count(&self) -> u32 {
        self.live_count
    }

    /// 非同期レッスン（動画添削）の完了したレッスンの数。
    pub const fn async_count(&self) -> u32 {
        self.async_count
    }

    /// 同期・非同期を合わせた完了したレッスンの数。
    pub const fn total_count(&self) -> u32 {
        self.live_count.saturating_add(self.async_count)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        async_lesson::VideoId,
        lesson::{LessonCompletionId, LessonSource},
        live_lesson::SessionId,
    };

    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);

    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn completion(id: u128, coach_id: AccountId, source: LessonSource) -> LessonCompletion {
        completion_at(id, coach_id, source, SystemTime::UNIX_EPOCH)
    }

    fn completion_at(
        id: u128,
        coach_id: AccountId,
        source: LessonSource,
        at: SystemTime,
    ) -> LessonCompletion {
        LessonCompletion::restore(
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            source,
            at,
            false,
        )
    }

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    #[test]
    fn no_completions_count_zero() {
        let count = CoachLessonCount::tally(COACH, &[]);
        assert_eq!(count.coach_id(), COACH);
        assert_eq!((count.live_count(), count.async_count()), (0, 0));
        assert_eq!(count.total_count(), 0);
    }

    #[test]
    fn counts_live_and_async_lessons_of_the_coach_separately() {
        let session = LessonSource::Live(SessionId::from_u128(10));
        let completions = [
            // グループレッスンの同じセッションでも生徒ごとに数える
            completion(1, COACH, session),
            completion(2, COACH, session),
            completion(3, COACH, LessonSource::Async(VideoId::from_u128(20))),
            completion(4, OTHER_COACH, LessonSource::Async(VideoId::from_u128(21))),
        ];
        let count = CoachLessonCount::tally(COACH, &completions);
        assert_eq!((count.live_count(), count.async_count()), (2, 1));
        assert_eq!(count.total_count(), 3, "同期と非同期の合計");
    }

    #[test]
    fn tally_recent_counts_only_lessons_completed_within_the_recent_period() {
        let second = Duration::from_secs(1);
        let session = LessonSource::Live(SessionId::from_u128(10));
        let completions = [
            completion_at(1, COACH, session, now()),
            // 30 日前ちょうどは含む
            completion_at(
                2,
                COACH,
                LessonSource::Async(VideoId::from_u128(20)),
                now() - 30 * DAY,
            ),
            // 30 日より前は数えない
            completion_at(
                3,
                COACH,
                LessonSource::Async(VideoId::from_u128(21)),
                now() - 30 * DAY - second,
            ),
            completion_at(
                4,
                OTHER_COACH,
                LessonSource::Async(VideoId::from_u128(22)),
                now(),
            ),
        ];
        let count = CoachLessonCount::tally_recent(COACH, &completions, now());
        assert_eq!(count.coach_id(), COACH);
        assert_eq!((count.live_count(), count.async_count()), (1, 1));
        assert_eq!(count.total_count(), 2);

        let later = CoachLessonCount::tally_recent(COACH, &completions, now() + 30 * DAY);
        assert_eq!(later.total_count(), 1, "期間は照会した時刻から数える");
    }
}
