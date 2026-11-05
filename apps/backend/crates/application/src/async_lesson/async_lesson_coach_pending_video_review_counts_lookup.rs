use std::collections::{BTreeMap, BTreeSet};

use domain::{async_lesson::CoachPendingVideoReviewCount, identity::AccountId};

use super::VideoRepository;
use crate::{Unavailable, clock::Clock, profile::CoachPendingVideoReviewCountsLookup};

/// profile の [`CoachPendingVideoReviewCountsLookup`] の async-lesson による実装。動画の保存先から返却待ちの動画を 1 回で
/// 読み（[`VideoRepository::videos_awaiting_review`]。`ListPendingVideoReviews` と同じ読み方）コーチごとに分けて、照会した
/// 時刻（[`Clock`]）で domain の [`CoachPendingVideoReviewCount::tally`]（`getCoachOverview` の
/// [`super::GetCoachPendingVideoReviewCount`] と同じ数え方）で数える。何も変えない。
#[derive(Debug, Clone)]
pub struct AsyncLessonCoachPendingVideoReviewCountsLookup<V, C> {
    videos: V,
    clock: C,
}

impl<V, C> AsyncLessonCoachPendingVideoReviewCountsLookup<V, C> {
    pub fn new(videos: V, clock: C) -> Self {
        Self { videos, clock }
    }
}

impl<V, C> CoachPendingVideoReviewCountsLookup
    for AsyncLessonCoachPendingVideoReviewCountsLookup<V, C>
where
    V: VideoRepository,
    C: Clock,
{
    async fn find_pending_video_review_counts(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> Result<BTreeMap<AccountId, CoachPendingVideoReviewCount>, Unavailable> {
        if coach_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let now = self.clock.now();
        let mut videos_of: BTreeMap<AccountId, Vec<_>> =
            coach_ids.iter().map(|id| (*id, Vec::new())).collect();
        for video in self.videos.videos_awaiting_review().await? {
            if let Some(videos) = videos_of.get_mut(&video.coach_id()) {
                videos.push(video);
            }
        }
        Ok(videos_of
            .into_iter()
            .map(|(coach_id, videos)| {
                (
                    coach_id,
                    CoachPendingVideoReviewCount::tally(coach_id, &videos, now),
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
        catalog::Sport,
    };

    use super::*;
    use crate::{
        async_lesson::GetCoachPendingVideoReviewCount,
        in_memory::{FixedClock, InMemoryVideoRepository},
    };

    const HOUR: Duration = Duration::from_secs(3600);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const NO_VIDEOS_COACH: AccountId = AccountId::from_u128(9);
    const UNASKED_COACH: AccountId = AccountId::from_u128(10);

    fn now() -> SystemTime {
        // 2026-10-31T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_404_800)
    }

    /// `id` の動画を生徒が `coach` に `hours` 時間前に送る。`returned` なら照会の時刻に返却済み。
    fn video(id: u128, coach: AccountId, hours: u32, returned: bool) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            STUDENT,
            coach,
            Sport::Tennis,
            now() - HOUR * hours,
            returned.then(|| {
                (
                    VideoReviewId::from_u128(id + 100),
                    ReviewComment::new("テイクバックが早い").unwrap(),
                    now(),
                )
            }),
            None,
        )
        .unwrap()
    }

    fn videos() -> InMemoryVideoRepository {
        InMemoryVideoRepository::with_videos([
            video(1, COACH, 10, false),
            video(2, COACH, 48, false),
            video(3, COACH, 200, true),
            video(4, OTHER_COACH, 47, false),
            video(5, OTHER_COACH, 300, true),
            // 聞いていないコーチの動画は数えない
            video(6, UNASKED_COACH, 72, false),
        ])
    }

    #[tokio::test]
    async fn counts_pending_videos_of_each_coach_the_same_as_one_coach() {
        let coach_ids: BTreeSet<_> = [COACH, OTHER_COACH, NO_VIDEOS_COACH].into();
        for at in [now(), now() + HOUR, now() + 38 * HOUR] {
            let counts =
                AsyncLessonCoachPendingVideoReviewCountsLookup::new(videos(), FixedClock(at))
                    .find_pending_video_review_counts(&coach_ids)
                    .await
                    .unwrap();
            assert_eq!(
                counts.keys().copied().collect::<BTreeSet<_>>(),
                coach_ids,
                "聞いたコーチをすべて返す（動画のないコーチも 0 件）"
            );
            for coach_id in &coach_ids {
                assert_eq!(
                    counts[coach_id],
                    GetCoachPendingVideoReviewCount::new(videos(), FixedClock(at))
                        .execute(*coach_id)
                        .await
                        .unwrap(),
                    "コーチ 1 人の様子の数と同じ: {coach_id:?} {at:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn counts_at_the_time_of_the_query() {
        let coach_ids: BTreeSet<_> = [COACH, OTHER_COACH, NO_VIDEOS_COACH].into();
        let overdue = |counts: BTreeMap<AccountId, CoachPendingVideoReviewCount>| {
            counts
                .values()
                .map(|count| (count.pending_count(), count.overdue_count()))
                .collect::<Vec<_>>()
        };
        let counts =
            AsyncLessonCoachPendingVideoReviewCountsLookup::new(videos(), FixedClock(now()))
                .find_pending_video_review_counts(&coach_ids)
                .await
                .unwrap();
        assert_eq!(
            overdue(counts),
            [(2, 1), (1, 0), (0, 0)],
            "返却済みは数えず、48 時間ちょうどは遅れに入り、47 時間は入らない"
        );
        let later =
            AsyncLessonCoachPendingVideoReviewCountsLookup::new(videos(), FixedClock(now() + HOUR))
                .find_pending_video_review_counts(&coach_ids)
                .await
                .unwrap();
        assert_eq!(
            overdue(later),
            [(2, 1), (1, 1), (0, 0)],
            "1 時間後には 47 時間前の動画も届く"
        );
        assert!(
            AsyncLessonCoachPendingVideoReviewCountsLookup::new(videos(), FixedClock(now()))
                .find_pending_video_review_counts(&BTreeSet::new())
                .await
                .unwrap()
                .is_empty()
        );
    }
}
