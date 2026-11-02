use std::collections::{BTreeMap, BTreeSet};

use domain::{
    identity::AccountId,
    lesson::{CoachLessonCount, RecentLessonPeriod},
};

use super::LessonCompletionRepository;
use crate::{Unavailable, clock::Clock, profile::CoachRecentLessonCountLookup};

/// profile の [`CoachRecentLessonCountLookup`] の、完了記録の保存先による実装。読むだけで、何も変えない。
///
/// コーチごとに期間の始まり（[`RecentLessonPeriod::since`]）以後の完了記録を読み、照会した時刻で domain の
/// [`CoachLessonCount::tally_recent`] で数える（数え方は [`super::GetMyLessonStats`] と同じ [`CoachLessonCount`]）。
#[derive(Debug, Clone)]
pub struct LessonCoachRecentLessonCountLookup<R, C> {
    completions: R,
    clock: C,
}

impl<R, C> LessonCoachRecentLessonCountLookup<R, C> {
    pub fn new(completions: R, clock: C) -> Self {
        Self { completions, clock }
    }
}

impl<R, C> CoachRecentLessonCountLookup for LessonCoachRecentLessonCountLookup<R, C>
where
    R: LessonCompletionRepository,
    C: Clock,
{
    async fn find_recent_lesson_counts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, CoachLessonCount>, Unavailable> {
        let now = self.clock.now();
        let since = RecentLessonPeriod::since(now);
        let mut counts = BTreeMap::new();
        for &coach_id in coach_ids {
            let completions = self.completions.find_by_coach(coach_id, since).await?;
            counts.insert(
                coach_id,
                CoachLessonCount::tally_recent(coach_id, &completions, now),
            );
        }
        Ok(counts)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::VideoId,
        lesson::{LessonCompletion, LessonCompletionId, LessonSource},
        live_lesson::SessionId,
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryLessonCompletionRepository};

    const DAY: Duration = Duration::from_secs(24 * 3600);
    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);
    const NO_LESSONS_COACH: AccountId = AccountId::from_u128(5);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn completion(id: u128, coach_id: AccountId, live: bool, at: SystemTime) -> LessonCompletion {
        let source = if live {
            LessonSource::Live(SessionId::from_u128(id))
        } else {
            LessonSource::Async(VideoId::from_u128(id))
        };
        LessonCompletion::restore(
            LessonCompletionId::from_u128(id),
            AccountId::from_u128(100 + id),
            coach_id,
            source,
            at,
            false,
        )
    }

    #[tokio::test]
    async fn counts_lessons_of_each_coach_completed_in_the_last_30_days() {
        let completions = InMemoryLessonCompletionRepository::with_completions([
            completion(1, COACH, true, now()),
            completion(2, COACH, false, now() - DAY),
            completion(3, COACH, false, now() - 30 * DAY),
            completion(4, COACH, true, now() - 31 * DAY),
            completion(5, OTHER_COACH, true, now() - 2 * DAY),
        ]);
        let counts =
            LessonCoachRecentLessonCountLookup::new(completions.clone(), FixedClock(now()))
                .find_recent_lesson_counts(&BTreeSet::from([COACH, OTHER_COACH, NO_LESSONS_COACH]))
                .await
                .unwrap();
        let totals: Vec<(AccountId, u32, u32)> = counts
            .iter()
            .map(|(&id, count)| (id, count.live_count(), count.async_count()))
            .collect();
        assert_eq!(
            totals,
            [(COACH, 1, 2), (OTHER_COACH, 1, 0), (NO_LESSONS_COACH, 0, 0)],
            "30 日前ちょうどは含み、31 日前は数えない。レッスンのないコーチも 0 件で含める"
        );

        // 期間は照会した時刻から数える
        let later =
            LessonCoachRecentLessonCountLookup::new(completions, FixedClock(now() + 29 * DAY))
                .find_recent_lesson_counts(&BTreeSet::from([COACH]))
                .await
                .unwrap();
        assert_eq!(later[&COACH].total_count(), 2);
    }
}
