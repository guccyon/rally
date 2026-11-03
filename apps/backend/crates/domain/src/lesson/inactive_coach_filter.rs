use super::CoachLessonCount;

/// 本部がコーチを「直近にレッスンのないコーチ」に絞るときの条件（admin-web の `listCoaches` の `inactive`）。
/// 「直近にレッスンのない」をどう決めるかはここだけに置く。
///
/// - `inactive` が `true`: 直近の期間（[`super::RecentLessonPeriod`]）に完了したレッスンの数（[`CoachLessonCount::tally_recent`]
///   で数えたもの。`listCoaches` の `recent_lesson_count` と同じ数）が 0 のコーチだけ
/// - `false`（既定）: 絞らない（どのコーチも通す）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InactiveCoachFilter(bool);

impl InactiveCoachFilter {
    /// `inactive` が `true` なら直近にレッスンのないコーチだけに絞る条件。`false` なら絞らない。
    pub const fn new(inactive: bool) -> Self {
        Self(inactive)
    }

    /// 直近にレッスンのないコーチだけに絞るか。
    pub const fn inactive(&self) -> bool {
        self.0
    }

    /// 直近の期間で数えた `recent_lessons` のコーチがこの条件に合うか。
    pub const fn matches(&self, recent_lessons: &CoachLessonCount) -> bool {
        !self.0 || recent_lessons.total_count() == 0
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::{
        async_lesson::VideoId,
        identity::AccountId,
        lesson::{LessonCompletion, LessonCompletionId, LessonSource},
        live_lesson::SessionId,
    };

    const COACH: AccountId = AccountId::from_u128(3);
    const OTHER_COACH: AccountId = AccountId::from_u128(4);
    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn completion(
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

    fn recent(completions: &[LessonCompletion]) -> CoachLessonCount {
        CoachLessonCount::tally_recent(COACH, completions, now())
    }

    #[test]
    fn inactive_matches_only_coaches_without_lessons_in_the_recent_period() {
        let filter = InactiveCoachFilter::new(true);
        assert!(filter.inactive());
        assert!(
            filter.matches(&recent(&[])),
            "完了したレッスンがなければ載る"
        );
        assert!(
            filter.matches(&recent(&[
                // 30 日より前のレッスンと、他のコーチのレッスンは数えない
                completion(
                    1,
                    COACH,
                    LessonSource::Async(VideoId::from_u128(20)),
                    now() - 30 * DAY - Duration::from_secs(1),
                ),
                completion(
                    2,
                    OTHER_COACH,
                    LessonSource::Live(SessionId::from_u128(10)),
                    now()
                ),
            ])),
            "期間の外のレッスンだけなら載る"
        );
        for source in [
            LessonSource::Live(SessionId::from_u128(10)),
            LessonSource::Async(VideoId::from_u128(20)),
        ] {
            assert!(
                !filter.matches(&recent(&[completion(1, COACH, source, now() - 30 * DAY)])),
                "30 日前ちょうどの同期・非同期のレッスンは数えるので載らない: {source:?}"
            );
        }
    }

    #[test]
    fn not_inactive_matches_every_coach() {
        let filter = InactiveCoachFilter::default();
        assert_eq!(filter, InactiveCoachFilter::new(false));
        assert!(!filter.inactive());
        assert!(filter.matches(&recent(&[])));
        assert!(filter.matches(&recent(&[completion(
            1,
            COACH,
            LessonSource::Async(VideoId::from_u128(20)),
            now(),
        )])));
    }
}
