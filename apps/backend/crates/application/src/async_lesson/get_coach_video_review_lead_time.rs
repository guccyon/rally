use domain::{async_lesson::VideoReviewLeadTime, identity::AccountId};

use super::VideoRepository;
use crate::{Unavailable, clock::Clock};

/// コーチ 1 人の、直近 30 日に送られた動画の返却までの時間の中央値（KPI「添削の返却リードタイム」の数え方。まだ返して
/// いない動画は今までの経過時間で数える）と、そのうち返した動画添削の件数のユースケース（本部の `getCoachOverview` が
/// 使う）。
///
/// コーチ `coach_id` に送られた動画を保存先から読み、今の時刻で domain の [`VideoReviewLeadTime::tally`] に数えさせる
/// （期間と数え方はここで書かない）。読むだけで何も変えない。誰が呼べるかは呼ぶ側の境界が確かめる。
pub struct GetCoachVideoReviewLeadTime<V, C> {
    videos: V,
    clock: C,
}

impl<V, C> GetCoachVideoReviewLeadTime<V, C>
where
    V: VideoRepository,
    C: Clock,
{
    pub fn new(videos: V, clock: C) -> Self {
        Self { videos, clock }
    }

    pub async fn execute(&self, coach_id: AccountId) -> Result<VideoReviewLeadTime, Unavailable> {
        let videos = self.videos.videos_of_coach(coach_id).await?;
        Ok(VideoReviewLeadTime::tally(
            coach_id,
            &videos,
            self.clock.now(),
        ))
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

    const COACH: AccountId = AccountId::from_u128(1);
    const OTHER_COACH: AccountId = AccountId::from_u128(2);
    const STUDENT: AccountId = AccountId::from_u128(3);
    const HOUR: Duration = Duration::from_secs(3600);
    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + DAY * 100
    }

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

    #[tokio::test]
    async fn counts_videos_sent_to_the_coach_in_the_last_30_days_as_of_the_clock() {
        let videos = InMemoryVideoRepository::with_videos([
            // 返却まで 2 時間・6 時間・10 時間と、まだ返していない 1 本（送ってから今まで 72 時間）。中央値 8 時間
            video(1, COACH, now() - DAY - HOUR * 2, Some(now() - DAY)),
            video(2, COACH, now() - DAY * 2 - HOUR * 6, Some(now() - DAY * 2)),
            video(
                3,
                COACH,
                now() - DAY * 29 - HOUR * 10,
                Some(now() - DAY * 29),
            ),
            video(4, COACH, now() - DAY * 3, None),
            // 31 日前に送った動画は、返却が期間内でも数えない。他のコーチの動画も数えない
            video(5, COACH, now() - DAY * 31, Some(now() - DAY)),
            video(6, OTHER_COACH, now() - HOUR * 2, Some(now() - HOUR)),
        ]);
        let use_case = GetCoachVideoReviewLeadTime::new(videos.clone(), FixedClock(now()));

        let lead_time = use_case.execute(COACH).await.unwrap();

        assert_eq!(lead_time.returned_count(), 3);
        assert_eq!(lead_time.median_hours(), Some(8.0));
        assert_eq!(videos.all().len(), 6, "読むだけで何も変えない");

        // 時計が進めば期間も、まだ返していない動画の経過時間も動く（29 日 10 時間前に送った分が外れ、未返却は 120 時間）
        let later = GetCoachVideoReviewLeadTime::new(videos, FixedClock(now() + DAY * 2));
        let lead_time = later.execute(COACH).await.unwrap();
        assert_eq!(lead_time.returned_count(), 2);
        assert_eq!(lead_time.median_hours(), Some(6.0));
    }

    #[tokio::test]
    async fn a_coach_without_videos_in_the_window_has_zero_and_no_median() {
        let videos =
            InMemoryVideoRepository::with_videos([video(1, COACH, now() - DAY * 31, None)]);
        let lead_time = GetCoachVideoReviewLeadTime::new(videos, FixedClock(now()))
            .execute(COACH)
            .await
            .unwrap();
        assert_eq!(
            (lead_time.returned_count(), lead_time.median_hours()),
            (0, None)
        );
    }
}
