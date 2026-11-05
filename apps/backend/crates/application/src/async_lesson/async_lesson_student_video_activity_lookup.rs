use std::{
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};

use domain::{async_lesson::PendingVideoReviewFilter, identity::AccountId};

use super::VideoRepository;
use crate::{
    Unavailable,
    identity::{
        StudentPendingVideoReviewCountsLookup, StudentVideoActivity, StudentVideoActivityLookup,
    },
};

/// identity の [`StudentVideoActivityLookup`] と [`StudentPendingVideoReviewCountsLookup`] の async-lesson による実装。
/// 動画の保存先から生徒の動画を読むだけで、何も変えない。
///
/// 送った動画の数は、生徒が送った動画すべての数（状態を問わず、期間で絞らない）。返却待ちの数は、そのうち
/// 本部が見る返却待ちの一覧（[`super::ListPendingVideoReviews`]）と同じ判定（domain の
/// [`PendingVideoReviewFilter`] の既定の条件。スポーツ・コーチ・経過時間で絞らない）に合う動画の数。
/// 生徒 1 人なら生徒の動画を読み（[`VideoRepository::videos_of_student`]）、複数の生徒なら返却待ちの動画を 1 回で読み
/// （[`VideoRepository::videos_awaiting_review`]。`ListPendingVideoReviews` と同じ読み方）生徒ごとに分けて、同じ判定で数える。
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
        Ok(StudentVideoActivity {
            submitted_count: count(videos.len())?,
            pending_review_count: count(pending)?,
        })
    }
}

impl<V> StudentPendingVideoReviewCountsLookup for AsyncLessonStudentVideoActivityLookup<V>
where
    V: VideoRepository,
{
    async fn find_pending_video_review_counts(
        &self,
        student_ids: &BTreeSet<AccountId>,
        now: SystemTime,
    ) -> Result<BTreeMap<AccountId, u32>, Unavailable> {
        if student_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut videos_of: BTreeMap<AccountId, Vec<_>> =
            student_ids.iter().map(|id| (*id, Vec::new())).collect();
        for video in self.videos.videos_awaiting_review().await? {
            if let Some(videos) = videos_of.get_mut(&video.student_id()) {
                videos.push(video);
            }
        }
        videos_of
            .into_iter()
            .map(|(student_id, videos)| {
                let pending = PendingVideoReviewFilter::default().count(&videos, now);
                Ok((student_id, count(pending)?))
            })
            .collect()
    }
}

fn count(count: usize) -> Result<u32, Unavailable> {
    u32::try_from(count).map_err(|_| Unavailable::new("動画の数が数えられる範囲を超えた"))
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
    async fn counts_pending_videos_of_each_student_the_same_as_one_student() {
        let lookup =
            AsyncLessonStudentVideoActivityLookup::new(InMemoryVideoRepository::with_videos([
                video(1, STUDENT, COACH, Sport::Tennis, 1, false),
                video(2, STUDENT, OTHER_COACH, Sport::Golf, 99, false),
                video(3, STUDENT, COACH, Sport::Tennis, 60, false),
                video(4, STUDENT, COACH, Sport::Tennis, 2, true),
                // 返却済みだけの生徒は 0 件
                video(7, OTHER_STUDENT, COACH, Sport::Tennis, 1, true),
                // 聞いていない生徒の動画は数えない
                video(8, AccountId::from_u128(9), COACH, Sport::Tennis, 1, false),
            ]));
        let third = AccountId::from_u128(3);
        let counts = lookup
            .find_pending_video_review_counts(&[STUDENT, OTHER_STUDENT, third].into(), at(50))
            .await
            .unwrap();
        assert_eq!(
            counts,
            [(STUDENT, 3), (OTHER_STUDENT, 0), (third, 0)].into(),
            "聞いた生徒をすべて返す（動画のない生徒も 0 件）"
        );
        for student_id in [STUDENT, OTHER_STUDENT, third] {
            assert_eq!(
                counts[&student_id],
                lookup
                    .find_video_activity(student_id, at(50))
                    .await
                    .unwrap()
                    .pending_review_count,
                "生徒 1 人の利用の様子の返却待ちの数と同じ: {student_id:?}"
            );
        }
        assert!(
            lookup
                .find_pending_video_review_counts(&BTreeSet::new(), at(50))
                .await
                .unwrap()
                .is_empty()
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
