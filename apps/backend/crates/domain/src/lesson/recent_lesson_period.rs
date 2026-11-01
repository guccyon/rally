use std::time::{Duration, SystemTime};

use super::LessonCompletion;

/// 本部が見る「直近に終わったレッスン」の期間（admin-web の `listCoaches` の `recent_lesson_count`。期間の正本は
/// `specs/contracts/api/admin-web/openapi.yaml` の `listCoaches`）。
///
/// 照会した時刻の [`RecentLessonPeriod::WINDOW_DAYS`] 日前（含む）から照会した時刻（含む）までに完了したレッスンを
/// 「直近に終わったレッスン」とする。期間はこの型の 1 か所だけに置き、数え方は [`super::CoachLessonCount::tally`] を使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentLessonPeriod;

impl RecentLessonPeriod {
    /// 見る期間の日数（直近 30 日）
    pub const WINDOW_DAYS: u32 = 30;
    /// 見る期間の長さ（[`Self::WINDOW_DAYS`] 日）
    pub const WINDOW: Duration = Duration::from_secs(Self::WINDOW_DAYS as u64 * 24 * 3600);

    /// `now` に照会したときの期間の始まり（含む）。`now` から期間を引けない（時刻の範囲の外）ときは UNIX 時刻の起点。
    pub fn since(now: SystemTime) -> SystemTime {
        now.checked_sub(Self::WINDOW)
            .unwrap_or(SystemTime::UNIX_EPOCH)
    }

    /// `completion` が `now` に照会したときの期間の中に完了したか。
    pub fn contains(completion: &LessonCompletion, now: SystemTime) -> bool {
        (Self::since(now)..=now).contains(&completion.completed_at())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        async_lesson::VideoId,
        identity::AccountId,
        lesson::{LessonCompletionId, LessonSource},
    };

    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn completed_at(at: SystemTime) -> LessonCompletion {
        LessonCompletion::restore(
            LessonCompletionId::from_u128(1),
            AccountId::from_u128(2),
            AccountId::from_u128(3),
            LessonSource::Async(VideoId::from_u128(4)),
            at,
            false,
        )
    }

    #[test]
    fn the_window_is_30_days_back_from_the_time_of_the_query() {
        assert_eq!(RecentLessonPeriod::WINDOW_DAYS, 30);
        assert_eq!(RecentLessonPeriod::WINDOW, 30 * DAY);
        assert_eq!(RecentLessonPeriod::since(now()), now() - 30 * DAY);
    }

    #[test]
    fn contains_both_ends_of_the_window() {
        let second = Duration::from_secs(1);
        for (at, expected) in [
            (now(), true),
            (now() - 30 * DAY, true),
            (now() - 30 * DAY - second, false),
            (now() + second, false),
        ] {
            assert_eq!(
                RecentLessonPeriod::contains(&completed_at(at), now()),
                expected,
                "{at:?}"
            );
        }
    }
}
