use std::{cmp::Ordering, collections::BTreeSet};

use domain::{async_lesson::Video, identity::AccountId};

use super::{CoachVideoList, VideoRepository, VideoStudentDisplayNameLookup};
use crate::Unavailable;

/// コーチに届いた動画の一覧のユースケース（admin-web の `listMyCoachVideos`。コーチだけが呼ぶ）。
///
/// コーチ `coach_id` に送られた動画を、添削待ち（未返却）を送られた時刻の古い順で先に、その後に返却済みを返却した
/// 時刻の新しい順に並べる（同じ時刻は動画 ID の順）。一覧に出る生徒の表示名は profile に問い合わせて
/// （[`VideoStudentDisplayNameLookup`]）添える。読むだけで何も変えない。
pub struct ListCoachVideos<V, D> {
    videos: V,
    students: D,
}

impl<V, D> ListCoachVideos<V, D>
where
    V: VideoRepository,
    D: VideoStudentDisplayNameLookup,
{
    pub fn new(videos: V, students: D) -> Self {
        Self { videos, students }
    }

    pub async fn execute(&self, coach_id: AccountId) -> Result<CoachVideoList, Unavailable> {
        let mut videos = self.videos.videos_of_coach(coach_id).await?;
        videos.sort_by(coach_video_order);
        let student_ids: BTreeSet<AccountId> = videos.iter().map(Video::student_id).collect();
        let students = if student_ids.is_empty() {
            Default::default()
        } else {
            self.students
                .find_student_display_names(&student_ids)
                .await?
        };
        Ok(CoachVideoList { videos, students })
    }
}

/// 添削待ちが先（送られた時刻の古い順）、返却済みが後（返却した時刻の新しい順）。同じ時刻は動画 ID の順。
fn coach_video_order(a: &Video, b: &Video) -> Ordering {
    match (a.review(), b.review()) {
        (None, None) => a.submitted_at().cmp(&b.submitted_at()),
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => b.returned_at().cmp(&a.returned_at()),
    }
    .then_with(|| a.id().cmp(&b.id()))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
        time::{Duration, SystemTime},
    };

    use domain::{
        async_lesson::{ReviewComment, VideoId, VideoReviewId},
        catalog::Sport,
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
        DisplayName::new("あおき").unwrap()
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

    /// `STUDENT` の表示名だけを知っていて（`OTHER_STUDENT` は引けない）、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl VideoStudentDisplayNameLookup for RecordingLookup {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.asked.lock().unwrap().push(student_ids.clone());
            Ok(student_ids
                .iter()
                .filter(|id| **id == STUDENT)
                .map(|id| (*id, name()))
                .collect())
        }
    }

    fn ids(list: &CoachVideoList) -> Vec<u128> {
        list.videos.iter().map(|v| v.id().as_u128()).collect()
    }

    #[tokio::test]
    async fn lists_awaiting_videos_oldest_first_then_returned_ones_latest_return_first() {
        let videos = InMemoryVideoRepository::with_videos([
            // 返却済み: 返却の新しい順に 6 → 5 → 7（送られた時刻の順ではない）
            video(5, STUDENT, COACH, 1, Some(20)),
            video(6, OTHER_STUDENT, COACH, 2, Some(30)),
            video(7, STUDENT, COACH, 0, Some(10)),
            // 添削待ち: 送られた時刻の古い順に 3 → 1 → 2（同じ時刻の 1 と 2 は動画 ID の順）
            video(2, OTHER_STUDENT, COACH, 4, None),
            video(1, STUDENT, COACH, 4, None),
            video(3, STUDENT, COACH, 3, None),
            // 別のコーチに送られた動画は出ない
            video(9, STUDENT, OTHER_COACH, 0, None),
        ]);
        let lookup = RecordingLookup::default();
        let list = ListCoachVideos::new(videos, lookup.clone())
            .execute(COACH)
            .await
            .unwrap();
        assert_eq!(ids(&list), [3, 1, 2, 6, 5, 7]);
        assert!(list.videos.iter().all(|v| v.coach_id() == COACH));
        assert_eq!(
            list.students,
            BTreeMap::from([(STUDENT, name())]),
            "表示名が引けない生徒（OTHER_STUDENT）は含まない"
        );
        assert_eq!(
            *lookup.asked.lock().unwrap(),
            [BTreeSet::from([STUDENT, OTHER_STUDENT])],
            "一覧の生徒を重ねずに 1 回で問い合わせる"
        );
    }

    #[tokio::test]
    async fn returned_videos_with_the_same_return_time_are_in_video_id_order() {
        let videos = InMemoryVideoRepository::with_videos([
            video(2, STUDENT, COACH, 0, Some(5)),
            video(1, STUDENT, COACH, 1, Some(5)),
        ]);
        let list = ListCoachVideos::new(videos, RecordingLookup::default())
            .execute(COACH)
            .await
            .unwrap();
        assert_eq!(ids(&list), [1, 2]);
    }

    #[tokio::test]
    async fn coach_without_videos_gets_an_empty_list_without_asking_for_display_names() {
        let videos =
            InMemoryVideoRepository::with_videos([video(1, STUDENT, OTHER_COACH, 0, None)]);
        let lookup = RecordingLookup::default();
        let list = ListCoachVideos::new(videos, lookup.clone())
            .execute(COACH)
            .await
            .unwrap();
        assert!(list.videos.is_empty());
        assert!(list.students.is_empty());
        assert!(lookup.asked.lock().unwrap().is_empty());
    }
}
