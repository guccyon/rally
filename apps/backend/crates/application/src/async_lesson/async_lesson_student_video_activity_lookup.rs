use std::time::SystemTime;

use domain::{async_lesson::PendingVideoReviewFilter, identity::AccountId};

use super::VideoRepository;
use crate::{
    Unavailable,
    identity::{StudentVideoActivity, StudentVideoActivityLookup},
};

/// identity の [`StudentVideoActivityLookup`] の async-lesson による実装。動画の保存先から生徒の動画を読むだけで、
/// 何も変えない。
///
/// 送った動画の数は、生徒が送った動画すべての数（状態を問わず、期間で絞らない）。返却待ちの数は、そのうち
/// 本部が見る返却待ちの一覧（[`super::ListPendingVideoReviews`]）と同じ判定（domain の
/// [`PendingVideoReviewFilter`] の既定の条件。スポーツ・コーチ・経過時間で絞らない）に合う動画の数。
#[derive(Debug, Clone)]
pub struct AsyncLessonStudentVideoActivityLookup<V> {
    videos: V,
}

impl<V> AsyncLessonStudentVideoActivityLookup<V> {
    pub fn new(videos: V) -> Self {
        Self { videos }
    }
}

impl<V> StudentVideoActivityLookup for AsyncLessonStudentVideoActivityLookup<V>
where
    V: VideoRepository,
{
    async fn find_video_activity(
        &self,
        student_id: AccountId,
        now: SystemTime,
    ) -> Result<StudentVideoActivity, Unavailable> {
        let videos = self.videos.videos_of_student(student_id).await?;
        let pending = PendingVideoReviewFilter::default().count(&videos, now);
        let count = |count: usize| {
            u32::try_from(count).map_err(|_| Unavailable::new("動画の数が数えられる範囲を超えた"))
        };
        Ok(StudentVideoActivity {
            submitted_count: count(videos.len())?,
            pending_review_count: count(pending)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::{
        async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
        catalog::Sport,
    };

    use super::*;
    use crate::in_memory::InMemoryVideoRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_812_800 + hour * 3600)
    }

    /// `id` の動画を `student` が `coach` に `sport` で `hour` に送る。`returned` なら送った 1 時間後に返却済み。
    fn video(
        id: u128,
        student: AccountId,
        coach: AccountId,
        sport: Sport,
        hour: u64,
        returned: bool,
    ) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            student,
            coach,
            sport,
            at(hour),
            returned.then(|| {
                (
                    VideoReviewId::from_u128(id + 100),
                    ReviewComment::new("テイクバックが早い").unwrap(),
                    at(hour + 1),
                )
            }),
            None,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn counts_all_submitted_videos_and_pending_ones_of_the_student() {
        let lookup =
            AsyncLessonStudentVideoActivityLookup::new(InMemoryVideoRepository::with_videos([
                // 返却待ち（コーチ・スポーツ・経過時間を問わない。照会より後に送った動画も 0 時間の返却待ち）
                video(1, STUDENT, COACH, Sport::Tennis, 1, false),
                video(2, STUDENT, OTHER_COACH, Sport::Golf, 99, false),
                video(3, STUDENT, COACH, Sport::Tennis, 60, false),
                // 返却済みは送った数にだけ入る
                video(4, STUDENT, COACH, Sport::Tennis, 2, true),
                video(5, STUDENT, OTHER_COACH, Sport::Tennis, 3, true),
                // 別の生徒の動画は数えない
                video(6, OTHER_STUDENT, COACH, Sport::Tennis, 1, false),
                video(7, OTHER_STUDENT, COACH, Sport::Tennis, 1, true),
            ]));
        assert_eq!(
            lookup.find_video_activity(STUDENT, at(50)).await.unwrap(),
            StudentVideoActivity {
                submitted_count: 5,
                pending_review_count: 3,
            }
        );
    }

    #[tokio::test]
    async fn student_without_videos_is_zero() {
        let lookup = AsyncLessonStudentVideoActivityLookup::new(InMemoryVideoRepository::default());
        assert_eq!(
            lookup.find_video_activity(STUDENT, at(0)).await.unwrap(),
            StudentVideoActivity::default()
        );
    }
}
