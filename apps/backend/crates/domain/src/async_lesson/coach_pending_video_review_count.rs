use std::time::SystemTime;

use super::{PendingVideoReviewFilter, Video};
use crate::identity::AccountId;

/// コーチ 1 人の返却待ちの動画添削の件数と、そのうち送付から [`Self::OVERDUE_HOURS`] 時間以上たった件数（admin-web の
/// `getCoachOverview` の `pending_video_review_count` / `overdue_pending_video_review_count`）。
///
/// 返却待ちかどうかと経過時間の数え方は [`PendingVideoReviewFilter::matches`]（`listPendingVideoReviews` と同じ 1 か所）で
/// 決め、ここでは数えるだけ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachPendingVideoReviewCount {
    coach_id: AccountId,
    pending_count: u32,
    overdue_count: u32,
}

impl CoachPendingVideoReviewCount {
    /// 返却が滞っているとみなす、送付からの経過時間の下限（時間。含む）。`listPendingVideoReviews` の
    /// `min_elapsed_hours=48` と同じ線。
    pub const OVERDUE_HOURS: u32 = 48;

    /// `videos` のうち、コーチ `coach_id` に送られ `now` に照会したときに返却待ちの動画を数える（他のコーチの動画・
    /// 返却済みの動画は数えない）。
    pub fn tally(coach_id: AccountId, videos: &[Video], now: SystemTime) -> Self {
        let pending = PendingVideoReviewFilter {
            coach_id: Some(coach_id),
            ..PendingVideoReviewFilter::default()
        };
        let overdue = PendingVideoReviewFilter {
            min_elapsed_hours: Self::OVERDUE_HOURS,
            ..pending
        };
        let count = |filter: PendingVideoReviewFilter| {
            let n = videos
                .iter()
                .filter(|video| filter.matches(video, now))
                .count();
            u32::try_from(n).unwrap_or(u32::MAX)
        };
        Self {
            coach_id,
            pending_count: count(pending),
            overdue_count: count(overdue),
        }
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// 返却待ちの動画添削の件数（経過時間を問わない）。
    pub const fn pending_count(&self) -> u32 {
        self.pending_count
    }

    /// 返却待ちのうち、送付から [`Self::OVERDUE_HOURS`] 時間以上たった件数。
    pub const fn overdue_count(&self) -> u32 {
        self.overdue_count
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        async_lesson::{ReviewComment, VideoId, VideoReviewId},
        catalog::Sport,
    };

    const HOUR: Duration = Duration::from_secs(3600);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(2);
    const OTHER_COACH: AccountId = AccountId::from_u128(3);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    fn pending(id: u128, coach: AccountId, sport: Sport, submitted_at: SystemTime) -> Video {
        Video::submit(VideoId::from_u128(id), STUDENT, coach, sport, submitted_at)
    }

    fn returned(id: u128, submitted_at: SystemTime) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            STUDENT,
            COACH,
            Sport::Tennis,
            submitted_at,
            Some((
                VideoReviewId::from_u128(id + 100),
                ReviewComment::new("テイクバックが早い").unwrap(),
                now(),
            )),
            None,
        )
        .unwrap()
    }

    #[test]
    fn no_videos_is_zero() {
        let count = CoachPendingVideoReviewCount::tally(COACH, &[], now());
        assert_eq!(count.coach_id(), COACH);
        assert_eq!((count.pending_count(), count.overdue_count()), (0, 0));
    }

    #[test]
    fn counts_pending_videos_of_the_coach_and_those_waiting_48_hours_or_more() {
        let videos = [
            pending(1, COACH, Sport::Tennis, now() - HOUR),
            // 48 時間ちょうどは含む
            pending(2, COACH, Sport::Tennis, now() - 48 * HOUR),
            // 47 時間 59 分 59 秒は含まない
            pending(
                3,
                COACH,
                Sport::Tennis,
                now() - 48 * HOUR + Duration::from_secs(1),
            ),
            // スポーツを問わない（ゴルフも数える）
            pending(4, COACH, Sport::Golf, now() - 100 * HOUR),
            // 返却済みは、どれだけ前に送られても数えない
            returned(5, now() - 200 * HOUR),
            // 他のコーチの動画は数えない
            pending(6, OTHER_COACH, Sport::Tennis, now() - 100 * HOUR),
        ];
        let count = CoachPendingVideoReviewCount::tally(COACH, &videos, now());
        assert_eq!(count.pending_count(), 4);
        assert_eq!(
            count.overdue_count(),
            2,
            "48 時間ちょうど（2）とゴルフ（4）"
        );
    }

    #[test]
    fn counts_agree_with_the_pending_video_review_list() {
        let videos = vec![
            pending(1, COACH, Sport::Tennis, now() - 10 * HOUR),
            pending(2, COACH, Sport::Soccer, now() - 72 * HOUR),
            pending(3, OTHER_COACH, Sport::Tennis, now() - 72 * HOUR),
            returned(4, now() - 72 * HOUR),
        ];
        let count = CoachPendingVideoReviewCount::tally(COACH, &videos, now());
        let listed = |min_elapsed_hours| {
            PendingVideoReviewFilter {
                sport: None,
                min_elapsed_hours,
                coach_id: Some(COACH),
            }
            .select(videos.clone(), now())
            .len()
        };
        assert_eq!(count.pending_count() as usize, listed(0));
        assert_eq!(
            count.overdue_count() as usize,
            listed(CoachPendingVideoReviewCount::OVERDUE_HOURS)
        );
    }
}
