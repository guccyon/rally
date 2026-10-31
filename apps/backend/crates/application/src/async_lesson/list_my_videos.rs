use std::{cmp::Reverse, collections::BTreeSet};

use domain::{async_lesson::Video, identity::AccountId};

use super::{MyVideoList, VideoCoachDisplayNameLookup, VideoRepository};
use crate::Unavailable;

/// 生徒が送った動画の一覧のユースケース（student-app の `listMyVideos`。生徒だけが呼ぶ）。
///
/// 生徒 `student_id` が送った動画を、添削待ちも返却済みも含めて送った時刻の新しい順に並べる（同じ時刻は動画 ID の
/// 順）。一覧に出る送り先のコーチの表示名は profile に問い合わせて（[`VideoCoachDisplayNameLookup`]）添える。
/// 読むだけで何も変えない。
pub struct ListMyVideos<V, D> {
    videos: V,
    coaches: D,
}

impl<V, D> ListMyVideos<V, D>
where
    V: VideoRepository,
    D: VideoCoachDisplayNameLookup,
{
    pub fn new(videos: V, coaches: D) -> Self {
        Self { videos, coaches }
    }

    pub async fn execute(&self, student_id: AccountId) -> Result<MyVideoList, Unavailable> {
        let mut videos = self.videos.videos_of_student(student_id).await?;
        videos.sort_by_key(|video| (Reverse(video.submitted_at()), video.id()));
        let coach_ids: BTreeSet<AccountId> = videos.iter().map(Video::coach_id).collect();
        let coaches = if coach_ids.is_empty() {
            Default::default()
        } else {
            self.coaches.find_coach_display_names(&coach_ids).await?
        };
        Ok(MyVideoList { videos, coaches })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
        time::{Duration, SystemTime},
    };

    use domain::{
        async_lesson::{ReviewComment, VideoId, VideoReviewId, VideoStatus},
        identity::Sport,
        profile::DisplayName,
    };

    use super::*;
    use crate::in_memory::InMemoryVideoRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn name() -> DisplayName {
        DisplayName::new("たなか").unwrap()
    }

    /// `id` の動画を `student` が `coach` に `submitted_hour` に送る。`returned_hour` があればその時刻に返却済み。
    fn video(
        id: u128,
        student: AccountId,
        coach: AccountId,
        submitted_hour: u64,
        returned_hour: Option<u64>,
    ) -> Video {
        Video::restore(
            VideoId::from_u128(id),
            student,
            coach,
            Sport::Tennis,
            at(submitted_hour),
            returned_hour.map(|hour| {
                (
                    VideoReviewId::from_u128(id + 100),
                    ReviewComment::new("テイクバックが早い").unwrap(),
                    at(hour),
                )
            }),
        )
        .unwrap()
    }

    /// `COACH` の表示名だけを知っていて（`OTHER_COACH` は引けない）、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl VideoCoachDisplayNameLookup for RecordingLookup {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.asked.lock().unwrap().push(coach_ids.clone());
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| (*id, name()))
                .collect())
        }
    }

    fn ids(list: &MyVideoList) -> Vec<u128> {
        list.videos.iter().map(|v| v.id().as_u128()).collect()
    }

    #[tokio::test]
    async fn lists_own_videos_latest_submission_first_including_returned_ones() {
        let videos = InMemoryVideoRepository::with_videos([
            // 返却済みも添削待ちも、送った時刻の新しい順に 3 → 1 → 2 → 4（同じ時刻の 1 と 2 は動画 ID の順。
            // 返却の時刻では並べない）
            video(4, STUDENT, COACH, 0, Some(50)),
            video(2, STUDENT, OTHER_COACH, 4, None),
            video(1, STUDENT, COACH, 4, Some(5)),
            video(3, STUDENT, COACH, 6, None),
            // 別の生徒が送った動画は出ない
            video(9, OTHER_STUDENT, COACH, 10, None),
        ]);
        let lookup = RecordingLookup::default();
        let list = ListMyVideos::new(videos, lookup.clone())
            .execute(STUDENT)
            .await
            .unwrap();
        assert_eq!(ids(&list), [3, 1, 2, 4]);
        assert!(list.videos.iter().all(|v| v.student_id() == STUDENT));
        assert_eq!(
            list.videos.iter().map(Video::status).collect::<Vec<_>>(),
            [
                VideoStatus::Submitted,
                VideoStatus::Reviewed,
                VideoStatus::Submitted,
                VideoStatus::Reviewed
            ]
        );
        assert_eq!(
            list.coaches,
            BTreeMap::from([(COACH, name())]),
            "表示名が引けないコーチ（OTHER_COACH）は含まない"
        );
        assert_eq!(
            *lookup.asked.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "一覧のコーチを重ねずに 1 回で問い合わせる"
        );
    }

    #[tokio::test]
    async fn student_without_videos_gets_an_empty_list_without_asking_for_display_names() {
        let videos =
            InMemoryVideoRepository::with_videos([video(1, OTHER_STUDENT, COACH, 0, None)]);
        let lookup = RecordingLookup::default();
        let list = ListMyVideos::new(videos, lookup.clone())
            .execute(STUDENT)
            .await
            .unwrap();
        assert!(list.videos.is_empty());
        assert!(list.coaches.is_empty());
        assert!(lookup.asked.lock().unwrap().is_empty());
    }
}
