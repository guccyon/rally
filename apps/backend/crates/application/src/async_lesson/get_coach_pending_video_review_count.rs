use domain::{async_lesson::CoachPendingVideoReviewCount, identity::AccountId};

use super::VideoRepository;
use crate::{Unavailable, clock::Clock};

/// コーチ 1 人の返却待ちの動画添削の件数と、そのうち送付から 48 時間以上たった件数のユースケース（admin-web の
/// `getCoachOverview` の項目。本部だけが呼ぶ）。照会だけで、何も保存しない。
///
/// コーチに送られた動画を読み、照会した時刻で domain の [`CoachPendingVideoReviewCount::tally`] で数える（返却待ちの判定と
/// 経過時間の数え方は domain の [`domain::async_lesson::PendingVideoReviewFilter`] だけ。`listPendingVideoReviews` と同じ線）。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
pub struct GetCoachPendingVideoReviewCount<V, C> {
    videos: V,
    clock: C,
}

impl<V, C> GetCoachPendingVideoReviewCount<V, C>
where
    V: VideoRepository,
    C: Clock,
{
    pub fn new(videos: V, clock: C) -> Self {
        Self { videos, clock }
    }

    pub async fn execute(
        &self,
        coach_id: AccountId,
    ) -> Result<CoachPendingVideoReviewCount, Unavailable> {
        let now = self.clock.now();
        let videos = self.videos.videos_of_coach(coach_id).await?;
        Ok(CoachPendingVideoReviewCount::tally(coach_id, &videos, now))
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
    use crate::in_memory::{FixedClock, InMemoryVideoRepository};

    const HOUR: Duration = Duration::from_secs(3600);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const NO_VIDEOS_COACH: AccountId = AccountId::from_u128(9);

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

    #[tokio::test]
    async fn counts_pending_videos_of_the_coach_at_the_time_of_the_query() {
        let videos = InMemoryVideoRepository::with_videos([
            video(1, COACH, 10, false),
            video(2, COACH, 48, false),
            video(3, COACH, 200, true),
            video(4, OTHER_COACH, 72, false),
        ]);

        let count = GetCoachPendingVideoReviewCount::new(videos.clone(), FixedClock(now()))
            .execute(COACH)
            .await
            .unwrap();
        assert_eq!(count.coach_id(), COACH);
        assert_eq!(
            (count.pending_count(), count.overdue_count()),
            (2, 1),
            "返却済み（3）と他のコーチ（4）は数えず、48 時間ちょうど（2）は 48 時間以上に入る"
        );

        // 経過は照会した時刻から数える
        let later =
            GetCoachPendingVideoReviewCount::new(videos.clone(), FixedClock(now() + HOUR * 38))
                .execute(COACH)
                .await
                .unwrap();
        assert_eq!(
            (later.pending_count(), later.overdue_count()),
            (2, 2),
            "38 時間後には 10 時間前の動画も 48 時間に届く"
        );

        let empty = GetCoachPendingVideoReviewCount::new(videos, FixedClock(now()))
            .execute(NO_VIDEOS_COACH)
            .await
            .unwrap();
        assert_eq!((empty.pending_count(), empty.overdue_count()), (0, 0));
    }
}
