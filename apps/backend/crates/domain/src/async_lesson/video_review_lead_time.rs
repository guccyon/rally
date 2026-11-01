use std::time::{Duration, SystemTime};

use super::Video;
use crate::identity::AccountId;

/// コーチ 1 人の、直近 [`Self::WINDOW`] に返した動画添削（glossary の video_review）の件数と、送付から返却までの時間の
/// 中央値（admin-web の `getCoachOverview`。丸め方の正本は `specs/contracts/api/admin-web/openapi.yaml` の `CoachOverview`）。
///
/// 1 本の時間は `specs/analytics/kpi.yaml` の `video_review_lead_time` と同じく、送付（`video.submitted`）から返却
/// （`video_review.returned`）までの時間で、中央値は件数が偶数なら真ん中の 2 つの平均。返していない動画は数えない。
/// 期間と数え方はこの型の 1 か所だけに置く。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoReviewLeadTime {
    returned_count: u32,
    median: Option<Duration>,
}

impl VideoReviewLeadTime {
    /// 数える期間（集計した時刻から遡る 30 日。`video_review_lead_time` の `window` と同じ長さ）。
    pub const WINDOW: Duration = Duration::from_secs(30 * 24 * 3600);

    /// `videos` のうち、コーチ `coach_id` に送られ、返却した時刻が `now` の [`Self::WINDOW`] 前から `now` まで（両端を
    /// 含む）の動画を数える（他のコーチの動画・返していない動画は数えない）。
    pub fn tally(coach_id: AccountId, videos: &[Video], now: SystemTime) -> Self {
        let since = now
            .checked_sub(Self::WINDOW)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let mut lead_times: Vec<Duration> = videos
            .iter()
            .filter(|video| video.coach_id() == coach_id)
            .filter_map(|video| {
                let returned_at = video.review()?.returned_at();
                (since <= returned_at && returned_at <= now).then(|| {
                    // 返却は送付の後（Video の不変条件）なので、差は負にならない
                    returned_at
                        .duration_since(video.submitted_at())
                        .unwrap_or(Duration::ZERO)
                })
            })
            .collect();
        lead_times.sort_unstable();
        let middle = lead_times.len() / 2;
        let median = match lead_times.len() {
            0 => None,
            n if n % 2 == 1 => Some(lead_times[middle]),
            _ => Some((lead_times[middle - 1] + lead_times[middle]) / 2),
        };
        Self {
            returned_count: u32::try_from(lead_times.len()).unwrap_or(u32::MAX),
            median,
        }
    }

    /// 期間内に返した動画添削の件数。
    pub const fn returned_count(&self) -> u32 {
        self.returned_count
    }

    /// 送付から返却までの時間の中央値。返したものがなければ `None`。
    pub const fn median(&self) -> Option<Duration> {
        self.median
    }

    /// 中央値を時間に直し、小数第 3 位で四捨五入して小数第 2 位までにした値。返したものがなければ `None`。
    pub fn median_hours(&self) -> Option<f64> {
        // 整数のまま 1/100 時間（36 秒）単位に四捨五入し、浮動小数点の誤差を丸めに持ち込まない
        const HUNDREDTH_HOUR_MILLIS: u128 = 36_000;
        self.median.map(|median| {
            let hundredths =
                (median.as_millis() * 2 + HUNDREDTH_HOUR_MILLIS) / (HUNDREDTH_HOUR_MILLIS * 2);
            hundredths as f64 / 100.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        async_lesson::{ReviewComment, VideoId, VideoReviewId},
        catalog::Sport,
    };

    const COACH: AccountId = AccountId::from_u128(1);
    const OTHER_COACH: AccountId = AccountId::from_u128(2);
    const STUDENT: AccountId = AccountId::from_u128(3);
    const HOUR: Duration = Duration::from_secs(3600);

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + VideoReviewLeadTime::WINDOW * 3
    }

    /// `coach_id` に `submitted_at` に送られ、`returned_at` に添削を返した（`None` なら返していない）動画。
    fn video(
        id: u128,
        coach_id: AccountId,
        submitted_at: SystemTime,
        returned_at: Option<SystemTime>,
    ) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            STUDENT,
            coach_id,
            Sport::Tennis,
            submitted_at,
            returned_at.map(|at| {
                (
                    VideoReviewId::from_u128(id),
                    ReviewComment::new("よい").unwrap(),
                    at,
                )
            }),
            None,
        )
        .unwrap()
    }

    /// `now()` の `returned_hours_ago` 時間前に、送ってから `lead_hours` 時間で返した動画。
    fn returned(id: u128, returned_hours_ago: u32, lead_hours: u32) -> Video {
        let returned_at = now() - HOUR * returned_hours_ago;
        video(
            id,
            COACH,
            returned_at - HOUR * lead_hours,
            Some(returned_at),
        )
    }

    #[test]
    fn nothing_returned_is_zero_with_no_median() {
        let videos = [
            video(1, COACH, now() - HOUR, None),
            returned(2, 24 * 31, 5),
            video(3, OTHER_COACH, now() - HOUR * 2, Some(now() - HOUR)),
        ];
        let lead_time = VideoReviewLeadTime::tally(COACH, &videos, now());
        assert_eq!(lead_time.returned_count(), 0);
        assert_eq!(lead_time.median(), None);
        assert_eq!(lead_time.median_hours(), None);
        assert_eq!(
            VideoReviewLeadTime::tally(COACH, &[], now()).returned_count(),
            0
        );
    }

    #[test]
    fn median_of_an_odd_count_is_the_middle_one() {
        let videos = [returned(1, 1, 30), returned(2, 2, 2), returned(3, 3, 10)];
        let lead_time = VideoReviewLeadTime::tally(COACH, &videos, now());
        assert_eq!(lead_time.returned_count(), 3);
        assert_eq!(lead_time.median(), Some(HOUR * 10));
        assert_eq!(lead_time.median_hours(), Some(10.0));
    }

    #[test]
    fn median_of_an_even_count_is_the_mean_of_the_middle_two() {
        let videos = [
            returned(1, 1, 1),
            returned(2, 2, 4),
            returned(3, 3, 7),
            returned(4, 4, 100),
        ];
        let lead_time = VideoReviewLeadTime::tally(COACH, &videos, now());
        assert_eq!(lead_time.returned_count(), 4);
        assert_eq!(lead_time.median(), Some(HOUR * 11 / 2));
        assert_eq!(lead_time.median_hours(), Some(5.5));
    }

    #[test]
    fn counts_only_reviews_of_the_coach_returned_within_the_window() {
        let window = VideoReviewLeadTime::WINDOW;
        let videos = [
            // 期間の始まりちょうどは数える（送付が期間より前でも、返却が期間内なら数える）
            video(1, COACH, now() - window - HOUR * 3, Some(now() - window)),
            // 期間の始まりの 1 秒前は数えない
            video(
                2,
                COACH,
                now() - window - HOUR,
                Some(now() - window - Duration::from_secs(1)),
            ),
            // 集計した時刻ちょうどは数え、それより後は数えない
            video(3, COACH, now() - HOUR, Some(now())),
            video(4, COACH, now(), Some(now() + Duration::from_secs(1))),
            // 返していない動画・他のコーチの動画は数えない
            video(5, COACH, now() - window, None),
            video(6, OTHER_COACH, now() - HOUR * 2, Some(now() - HOUR)),
        ];
        let lead_time = VideoReviewLeadTime::tally(COACH, &videos, now());
        assert_eq!(lead_time.returned_count(), 2);
        assert_eq!(lead_time.median(), Some(HOUR * 2));
    }

    #[test]
    fn a_review_returned_at_submission_counts_as_zero_hours() {
        let videos = [video(1, COACH, now() - HOUR, Some(now() - HOUR))];
        let lead_time = VideoReviewLeadTime::tally(COACH, &videos, now());
        assert_eq!(lead_time.returned_count(), 1);
        assert_eq!(lead_time.median_hours(), Some(0.0));
    }

    #[test]
    fn median_hours_round_half_up_at_the_third_decimal() {
        let hours_of = |secs: u64| {
            let submitted_at = now() - HOUR * 24;
            let videos = [video(
                1,
                COACH,
                submitted_at,
                Some(submitted_at + Duration::from_secs(secs)),
            )];
            VideoReviewLeadTime::tally(COACH, &videos, now()).median_hours()
        };
        // 1/100 時間は 36 秒。その半分（18 秒）で切り上げる
        assert_eq!(hours_of(3600 + 17), Some(1.0));
        assert_eq!(hours_of(3600 + 18), Some(1.01));
        assert_eq!(hours_of(2 * 3600 + 20 * 60), Some(2.33));
        assert_eq!(hours_of(2 * 3600 + 40 * 60), Some(2.67));
    }
}
