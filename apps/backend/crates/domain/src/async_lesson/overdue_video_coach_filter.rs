use super::CoachPendingVideoReviewCount;

/// 本部がコーチを「返却の遅れている動画のあるコーチ」に絞るときの条件（admin-web の `listCoaches` の
/// `has_overdue_video`）。「返却の遅れている動画がある」をどう決めるかはここだけに置く。
///
/// - `has_overdue_video` が `true`: 返却待ちのうち送付から [`CoachPendingVideoReviewCount::OVERDUE_HOURS`] 時間以上たった
///   動画の数（[`CoachPendingVideoReviewCount::overdue_count`]。`getCoachOverview` の `overdue_pending_video_review_count`
///   と同じ数で、`listPendingVideoReviews` にそのコーチの `coach_id` と `min_elapsed_hours=48` を付けたときに載る動画の数）が
///   1 件以上のコーチだけ
/// - `false`（既定）: 絞らない（どのコーチも通す）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OverdueVideoCoachFilter(bool);

impl OverdueVideoCoachFilter {
    /// `has_overdue_video` が `true` なら返却の遅れている動画のあるコーチだけに絞る条件。`false` なら絞らない。
    pub const fn new(has_overdue_video: bool) -> Self {
        Self(has_overdue_video)
    }

    /// 返却の遅れている動画のあるコーチだけに絞るか。
    pub const fn has_overdue_video(&self) -> bool {
        self.0
    }

    /// 照会した時刻で数えた `pending_reviews` のコーチがこの条件に合うか。
    pub const fn matches(&self, pending_reviews: &CoachPendingVideoReviewCount) -> bool {
        !self.0 || pending_reviews.overdue_count() > 0
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::{
        async_lesson::{PendingVideoReviewFilter, ReviewComment, Video, VideoId, VideoReviewId},
        catalog::Sport,
        identity::AccountId,
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

    fn pending(id: u128, coach: AccountId, submitted_at: SystemTime) -> Video {
        Video::submit(
            VideoId::from_u128(id),
            STUDENT,
            coach,
            Sport::Tennis,
            submitted_at,
        )
    }

    fn returned(id: u128, coach: AccountId, submitted_at: SystemTime) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            STUDENT,
            coach,
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

    fn tally(videos: &[Video]) -> CoachPendingVideoReviewCount {
        CoachPendingVideoReviewCount::tally(COACH, videos, now())
    }

    #[test]
    fn has_overdue_video_matches_only_coaches_with_an_overdue_video() {
        let filter = OverdueVideoCoachFilter::new(true);
        assert!(filter.has_overdue_video());
        assert!(!filter.matches(&tally(&[])), "動画がなければ載らない");
        assert!(
            filter.matches(&tally(&[
                pending(1, COACH, now() - HOUR),
                // 48 時間ちょうど前の送付は数える
                pending(2, COACH, now() - 48 * HOUR),
            ])),
            "48 時間以上返却を待っている動画が 1 件でもあれば載る"
        );
        assert!(
            !filter.matches(&tally(&[
                // 48 時間に 1 秒足りない・返却済み・他のコーチの動画は数えない
                pending(1, COACH, now() - 48 * HOUR + SECOND),
                returned(2, COACH, now() - 200 * HOUR),
                pending(3, OTHER_COACH, now() - 72 * HOUR),
            ])),
            "返却待ちでも遅れていない動画だけなら載らない"
        );
    }

    #[test]
    fn has_overdue_video_is_the_same_as_the_overdue_pending_list_of_the_coach_being_non_empty() {
        let videos = vec![
            pending(1, COACH, now() - 10 * HOUR),
            returned(2, COACH, now() - 72 * HOUR),
            pending(3, OTHER_COACH, now() - 48 * HOUR),
        ];
        let filter = OverdueVideoCoachFilter::new(true);
        for coach_id in [COACH, OTHER_COACH, STUDENT] {
            for at in [now(), now() - SECOND, now() + 38 * HOUR] {
                let listed = PendingVideoReviewFilter {
                    coach_id: Some(coach_id),
                    min_elapsed_hours: CoachPendingVideoReviewCount::OVERDUE_HOURS,
                    ..PendingVideoReviewFilter::default()
                }
                .select(videos.clone(), at);
                let count = CoachPendingVideoReviewCount::tally(coach_id, &videos, at);
                assert_eq!(
                    filter.matches(&count),
                    !listed.is_empty(),
                    "{coach_id:?} {at:?}"
                );
            }
        }
    }

    #[test]
    fn not_has_overdue_video_matches_every_coach() {
        let filter = OverdueVideoCoachFilter::default();
        assert_eq!(filter, OverdueVideoCoachFilter::new(false));
        assert!(!filter.has_overdue_video());
        assert!(filter.matches(&tally(&[])));
        assert!(filter.matches(&tally(&[pending(1, COACH, now() - 72 * HOUR)])));
    }
}
